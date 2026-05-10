//! Play action — bridges a Kitsu-resolved title to the actual stream.
//!
//! The renderer's detail page calls `POST /api/play` (or its sibling
//! `/api/play/external`) with the canonical title + episode + mode.
//! Both endpoints walk the same chain:
//!
//!   1. Spawn `ani-cli -S <title> -e <episode>` via [`run_debug`].
//!      ani-cli internally searches allanime, picks the first match,
//!      resolves the chosen quality stream, and prints the result.
//!   2. Take the parsed [`DebugOutput`] and either
//!        - wrap the upstream URL in a [`StreamSession`] (embedded),
//!        - or hand it off to the user's `mpv` (external).
//!
//! No title-match cache yet — every play hits ani-cli fresh. The cache
//! is task #51 and lands once the spawn cost actually bites the UX.

use std::time::Duration;

use serde::Deserialize;

use crate::anicli::parser::{parse_progress_line, ProgressLine};
use crate::anicli::process::{run_debug, run_debug_streaming, DebugOptions};
use crate::app::AppState;
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::{
    external_player::{self, LaunchArgs},
    session::{create_session_with_kind, CreateSessionArgs, CreateSessionResponse},
};
use crate::config::read_config;
use crate::error::{AniError, Result};
use crate::proxy::{upstream, MediaKind};
use crate::scraper;
use crate::scraper::Candidate;

/// Frontend → backend payload for both play endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayArgs {
    /// Canonical title from the Kitsu metadata. Fed to ani-cli's
    /// search step (after we've picked the right candidate index).
    pub title: String,
    /// Episode number, as a string to match the CLI's positional arg
    /// shape (`-e 5` accepts `"5"` literally).
    pub episode: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// `"best"` / `"worst"` / `"1080"` / etc. Defaults to `"best"`.
    #[serde(default)]
    pub quality: Option<String>,
    /// Kitsu's authoritative episode count. Used to disambiguate
    /// allanime candidates that share a title (e.g. the 1-ep
    /// "Konoha Gakuen Den" side-story vs. the 500-ep main "Naruto:
    /// Shippuuden"). When `None`, we fall back to the legacy `-S 1`
    /// behaviour.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Fallback titles to try when the canonical title returns no
    /// allanime hits. Frontend feeds Kitsu's `titles.en_jp` /
    /// `titles.ja_jp` here so the play flow can recover when Kitsu's
    /// canonicalTitle is the English form (e.g. "JoJo's Bizarre
    /// Adventure: Stone Ocean") but allmanga only indexes the
    /// romanized name. Tried in order.
    ///
    /// Wire formats accepted (driven by `deserialize_alt_titles`):
    /// - JSON array (POST /api/play body): `["a","b"]`
    /// - Newline-joined string (SSE GET /api/play/stream query): `"a\nb"`.
    ///   Required because EventSource is GET-only and serde_urlencoded
    ///   doesn't handle repeated keys.
    #[serde(
        default,
        deserialize_with = "crate::commands::play_select::deserialize_alt_titles"
    )]
    pub alt_titles: Vec<String>,
    /// `true` when this call is a background prefetch (warming the
    /// cache for an episode the user hasn't clicked yet). Prefetches
    /// must NOT touch `ani-hsts` — the page-mount loop fires 12+ play
    /// calls in parallel and whichever resolves last would overwrite
    /// the user's actual click. The flag drives both:
    ///   - skipping our cache-hit history write
    ///   - redirecting ani-cli's `$ANI_CLI_HIST_DIR` to a tempdir so
    ///     ani-cli's own `update_history` writes to a throwaway file
    ///
    /// Frontend prefetch loops set it; click handlers leave it false.
    #[serde(
        default,
        deserialize_with = "crate::commands::play_select::deserialize_loose_bool"
    )]
    pub prefetch: bool,
    /// Kitsu id of the anime the user is playing. The frontend knows
    /// it (the user came from `/anime/[kitsu_id]`); we don't, until
    /// the user passes it in. Recording the
    /// (allmanga show_id → kitsu_id) pair on every successful play
    /// turns the home-page Continue Watching lookup from "fuzzy
    /// kitsuSearch on a possibly-typo'd allmanga title" into a
    /// deterministic id-keyed lookup. Empty string when the caller
    /// has no kitsu_id available (e.g. the SSE fallback path or a
    /// direct API user).
    #[serde(default)]
    pub kitsu_id: Option<String>,
}

// `deserialize_alt_titles`, `deserialize_loose_bool`, and the
// `select_first_with_hits*` family live in `commands::play_select`
// so this module's cyclomatic complexity stays manageable. The
// PlayArgs serde derive uses fully-qualified paths above, and the
// rest of the play flow imports them via the `use` line at the top
// of the file.
pub use crate::commands::play_select::{
    select_first_with_hits, select_first_with_hits_opt, select_first_with_hits_with_candidate,
};

/// Spawn timeout for the ani-cli search+resolve step. Real-world
/// allanime queries take 5-30s; 60s is a comfortable upper bound
/// before the user is better served by an error than a stuck spinner.
const RUN_DEBUG_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolve which `(title, 1-based candidate index)` to pass to
/// `ani-cli -S`. Calls our own allanime search for the canonical
/// title first; if that returns zero hits, walks `args.alt_titles`
/// in order until one returns a non-empty list, then runs
/// `pick_by_ep_count` over the winner. Falls through to
/// `(args.title, 1)` (legacy behaviour) on every-list-empty or when
/// `episode_count` is unknown.
pub(super) async fn pick_title_and_index(
    state: &AppState,
    args: &PlayArgs,
) -> (String, usize, Option<Candidate>) {
    let primary = args.title.clone();
    let mode = if args.mode == "dub" { "dub" } else { "sub" };

    // Walk the candidate list whether or not we have a Kitsu
    // episode_count to disambiguate with — alt_titles is also the
    // recovery path when canonical doesn't appear in allmanga's index
    // (Stone Ocean Part 6 reproduces this even though its
    // episode_count is null on Kitsu). Stop at the first non-empty
    // list so we don't make three GraphQL calls when canonical worked.
    let mut results: Vec<(String, Vec<Candidate>)> = Vec::new();
    for title in
        std::iter::once(args.title.as_str()).chain(args.alt_titles.iter().map(String::as_str))
    {
        match scraper::search(&state.proxy_http, title, mode, None).await {
            Ok(cands) => {
                tracing::info!(title, hits = cands.len(), "play: allanime search candidate",);
                let was_empty = cands.is_empty();
                results.push((title.to_string(), cands));
                if !was_empty {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(
                    title,
                    error = ?e,
                    "play: allanime search failed; trying next candidate",
                );
                results.push((title.to_string(), Vec::new()));
            }
        }
    }

    let (chosen_title, pick, chosen) =
        select_first_with_hits_with_candidate(&primary, &results, args.episode_count, mode);
    tracing::info!(
        primary = %primary,
        alt_count = args.alt_titles.len(),
        chosen_title = %chosen_title,
        expected_eps = ?args.episode_count,
        pick = pick,
        chosen_show_id = chosen.as_ref().map(|c| c.id.as_str()).unwrap_or(""),
        "play: chose ani-cli search title",
    );
    (chosen_title, pick, chosen)
}

/// Build the spawn options for an ani-cli invocation. When
/// `override_hist_dir` is `Some`, ani-cli writes its `ani-hsts` to that
/// path instead of the user's real history file — used by the prefetch
/// path to keep background warming out of Continue Watching.
pub(super) fn debug_options_for(
    state: &AppState,
    override_hist_dir: Option<&std::path::Path>,
) -> DebugOptions {
    let hist_dir = override_hist_dir
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            state
                .history_path
                .parent()
                .map(std::path::Path::to_path_buf)
        });
    DebugOptions {
        ani_cli_path: state.ani_cli_path.clone(),
        hist_dir,
        timeout: RUN_DEBUG_TIMEOUT,
        // None → inherit the backend process's PATH. Tests inject a
        // shimmed PATH by calling `run_debug` directly with their own
        // `DebugOptions` rather than going through the play handlers.
        path_override: None,
    }
}

/// Resolve `args` against ani-cli, register a stream session for the
/// resulting upstream URL, and return the proxy URLs hls.js will
/// consume.
///
/// # Errors
/// Inherits from [`run_debug`] (timeout, parse failure, scraper
/// errors) and [`create_session`] (URL-shape validation on the
/// resolved upstream).
pub async fn play(state: &AppState, args: &PlayArgs) -> Result<CreateSessionResponse> {
    play_with_progress(state, args, |_| {}).await
}

/// Like [`play`], but invokes `on_progress` once for every parsed
/// `ani-cli` stderr line as the resolution runs. Used by the SSE
/// `/api/play/stream` endpoint to forward incremental status to the
/// renderer's loading overlay.
///
/// The callback runs on the same async task as the resolution; a slow
/// callback stalls the subprocess. SSE handlers should push events
/// through an `mpsc` channel inside the callback rather than do work
/// inline.
///
/// # Errors
/// Same as [`play`].
pub async fn play_with_progress<F>(
    state: &AppState,
    args: &PlayArgs,
    mut on_progress: F,
) -> Result<CreateSessionResponse>
where
    F: FnMut(ProgressLine) + Send,
{
    // Per-call scratch dir for ani-cli's history write when this is a
    // prefetch — keeps background warming out of the user's real
    // ani-hsts. Held across the await so the dir lives until ani-cli
    // exits; auto-cleaned on drop.
    let prefetch_hist_dir = if args.prefetch {
        Some(tempfile::tempdir().map_err(|_| AniError::Io)?)
    } else {
        None
    };
    let opts = debug_options_for(state, prefetch_hist_dir.as_ref().map(|d| d.path()));
    let quality = args.quality.as_deref().unwrap_or("best");

    // Long-term cache check. A successful prior resolution under the
    // same (title, mode, quality, episode) tuple is replayable for up
    // to PLAY_RESOLUTION_TTL — we just have to confirm the upstream
    // URL is still alive (wixmp / sharepoint URLs rotate). HEAD is
    // ~50ms; ani-cli is ~30s. Worth the round-trip.
    let cache_key =
        play_resolution_cache::cache_key(&args.title, &args.mode, quality, &args.episode);
    if let Ok(Some(cached)) = play_resolution_cache::get(&state.cache_pool, &cache_key) {
        if let Some(resp) = try_serve_cached(state, &cached).await {
            tracing::info!(
                title = %args.title,
                episode = %args.episode,
                upstream = cached.upstream_url.as_str(),
                "play: cache hit (HEAD ok)",
            );
            // Update Continue Watching: the cache-miss path got history
            // for free via ani-cli's `update_history`. We don't run
            // ani-cli on a hit, so we do it ourselves. Skipped silently
            // for legacy rows (show_id empty) and for prefetch calls
            // (background warming must not bump the user's last-played
            // episode — prefetches resolve in arbitrary order).
            if !args.prefetch && !cached.show_id.is_empty() {
                let entry = crate::history::HistoryEntry {
                    ep_no: args.episode.clone(),
                    id: cached.show_id.clone(),
                    title: cached.show_title.clone(),
                };
                if let Err(e) = crate::history::upsert_and_write(&state.history_path, entry) {
                    tracing::warn!(
                        title = %args.title,
                        episode = %args.episode,
                        error = ?e,
                        "play: history write failed on cache hit",
                    );
                }
            }
            return Ok(resp);
        }
        // HEAD failed — the cached URL is dead. Evict the row and
        // fall through to ani-cli. Eviction is explicit (not just
        // overwrite-on-put) because if the fresh ani-cli call ALSO
        // fails, we don't want the stale row to linger and bite the
        // next attempt.
        play_resolution_cache::evict(&state.cache_pool, &cache_key);
        tracing::info!(
            title = %args.title,
            episode = %args.episode,
            "play: cache row stale (HEAD failed), evicted, falling back to ani-cli",
        );
    }

    // Pick which (title, candidate index) ani-cli should use. The title
    // may differ from args.title when alt_titles produced the winning
    // hit (e.g. romanized fallback for shows whose Kitsu canonicalTitle
    // is the English form). See pick_title_and_index().
    let (search_title, select_index, chosen_candidate) = pick_title_and_index(state, args).await;

    tracing::info!(
        search_title = %search_title,
        episode = %args.episode,
        select_index = select_index,
        mode = %args.mode,
        quality = quality,
        "play: spawning ani-cli",
    );

    let resolved = run_debug_streaming(
        &opts,
        &search_title,
        &args.episode,
        quality,
        &args.mode,
        select_index,
        |line| {
            // Mirror every ani-cli stderr line into our own logs so a
            // failed play has a paper trail. parse_progress_line still
            // runs on the same line for the SSE overlay.
            tracing::info!(line = %line, "anicli.stderr");
            if let Some(p) = parse_progress_line(line) {
                on_progress(p);
            }
        },
    )
    .await
    .inspect_err(|e| {
        // Log explicitly so `RUST_LOG=ani_gui=info` surfaces the
        // actual reason instead of leaving the user staring at an
        // overlay that flashed and disappeared. The `?` would
        // propagate it but no logger between here and the SSE
        // serializer prints it.
        tracing::error!(
            search_title = %search_title,
            episode = %args.episode,
            select_index = select_index,
            error = ?e,
            "play: ani-cli step failed",
        );
        // Persist the negative outcome for the availability cache so
        // home/search list filters learn from this click without an
        // extra round-trip. NoResults = not in catalogue, period.
        if matches!(e, AniError::NoResults) {
            if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
                crate::commands::availability::write_cache(state, id, &args.mode, false);
            }
        }
    })?;
    // Successful resolve = available. Enrich the cache row with the
    // playable episode count + recap extras so the next detail/play
    // visit doesn't have to re-probe. fetch_show is one extra
    // GraphQL round-trip against allmanga (~200ms) — invisible
    // here because the user is past ani-cli's multi-second
    // resolution and about to navigate to /play. Failure falls
    // through to the simple write so the row at least records
    // `available=true`.
    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        if let Some(c) = chosen_candidate.as_ref() {
            let mode_str = args.mode.as_str();
            let (episode_count, extras) =
                match crate::scraper::allanime::fetch_show(&state.proxy_http, &c.id, None).await {
                    Ok(detail) => {
                        let cap = detail.max_integer_episode(mode_str);
                        let ex: Vec<String> = detail
                            .available_episodes_detail
                            .for_mode(mode_str)
                            .iter()
                            .filter(|t| t.parse::<u32>().is_err())
                            .cloned()
                            .collect();
                        (cap, ex)
                    }
                    Err(_) => (None, Vec::new()),
                };
            // Status unknown at this layer (PlayArgs doesn't carry
            // it). None → write_cache_full uses the conservative
            // ongoing TTL (24h); the next detail-page probe knows
            // status and will overwrite with the right TTL.
            crate::commands::availability::write_cache_full(
                state,
                id,
                mode_str,
                true,
                episode_count,
                extras,
                None,
            );
        } else {
            crate::commands::availability::write_cache(state, id, &args.mode, true);
        }
    }

    // Decide media kind: cheap path-extension first, HEAD fallback
    // when the URL is opaque (fast4speed.rsvp/<id>/sub/1, etc).
    let upstream_url =
        url::Url::parse(&resolved.selected_url).map_err(|_| AniError::ParseFailed {
            detail: format!("upstream_url: {} is not a valid URL", resolved.selected_url),
        })?;

    // Infer Referer when ani-cli's debug output didn't include one.
    // Mirrors `refr_flag` switch in ani-cli (line ~209): the
    // tools.fast4speed.rsvp CDN enforces Referer = https://allmanga.to
    // and 403s requests without it. ani-cli sets the header internally
    // when invoking the player but doesn't surface it on stdout, so
    // the parser sees None for these URLs.
    let referer = match resolved.referer {
        Some(r) if !r.is_empty() => r,
        _ => match upstream_url.host_str() {
            Some(h) if h.ends_with("fast4speed.rsvp") => "https://allmanga.to".to_string(),
            _ => String::new(),
        },
    };

    let kind = match MediaKind::from_url(&upstream_url) {
        Some(k) => k,
        None => {
            // HEAD failures fall back to MP4 — that's the safe default
            // (binary streams, unknown CDNs). The proxy then serves
            // /file.mp4 with byte-range support; if the upstream truly
            // is an HLS manifest mislabelled, hls.js never enters the
            // picture and the renderer surfaces a real error.
            upstream::classify_via_head(&state.proxy_http, &upstream_url, &referer)
                .await
                .unwrap_or(MediaKind::Mp4)
        }
    };
    tracing::info!(
        title = %args.title,
        episode = %args.episode,
        upstream = upstream_url.as_str(),
        referer = referer.as_str(),
        kind = ?kind,
        "play: ani-cli resolved upstream",
    );

    // Persist the resolution so the next play of the same episode
    // skips ani-cli entirely (subject to TTL + HEAD validation).
    // show_id + show_title come from the chosen allanime candidate
    // (when our search picked one) so a future cache-hit can write to
    // ani-hsts ourselves — ani-cli's update_history doesn't fire when
    // we skip the subprocess on a cache hit.
    let (show_id, show_title) = chosen_candidate
        .as_ref()
        .map(|c| {
            (
                c.id.clone(),
                format!(
                    "{} ({} episodes)",
                    c.name,
                    c.available_episodes.for_mode(&args.mode)
                ),
            )
        })
        .unwrap_or_default();
    let cached_resolution = CachedResolution {
        upstream_url: resolved.selected_url.clone(),
        referer: referer.clone(),
        subtitle_url: resolved.subtitle_url.clone(),
        media_kind: kind,
        show_id,
        show_title,
    };
    play_resolution_cache::put(&state.cache_pool, &cache_key, &cached_resolution);

    let session_args = CreateSessionArgs {
        upstream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
    };
    create_session_with_kind(state, &session_args, kind)
}

// `upstream_head_ok`, `try_serve_cached`, and
// `try_launch_args_from_cache` live in `commands::play_cache` so
// this module's reported CCN stays under the CRAP ratchet's
// per-file limit. The tests in this file's `#[cfg(test)]` module
// still drive them via wiremock; they just import from the new
// module rather than calling sibling functions.
use crate::commands::play_cache::{try_launch_args_from_cache, try_serve_cached};

/// Resolve `args` against ani-cli and hand the upstream URL straight
/// to the user's external player (default `mpv`). No session is
/// registered — the player streams from the upstream directly with
/// the `Referer:` flag.
///
/// # Errors
/// Inherits from [`run_debug`] and
/// [`external_player::open_external_player`] (missing binary,
/// non-zero spawn status).
pub async fn play_external(state: &AppState, args: &PlayArgs) -> Result<()> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Long-term cache reuse — same shape as play_with_progress. The
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple seconds ago; without this the user
    // would wait another 30s for ani-cli to spin up a fresh fetch.
    // HEAD-validate so a stale/dead URL falls through to the fresh
    // path instead of handing mpv a 403.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return external_player::open_external_player(&launch);
    }

    // play_external is always a click — never a prefetch — so no
    // hist_dir override needed.
    let opts = debug_options_for(state, None);
    let (search_title, select_index, _chosen_candidate) = pick_title_and_index(state, args).await;
    let resolved = run_debug(
        &opts,
        &search_title,
        &args.episode,
        quality,
        &args.mode,
        select_index,
    )
    .await?;

    // Same Referer-inference as the embedded path — fast4speed.rsvp
    // 403s without `Referer: https://allmanga.to` and ani-cli's debug
    // output doesn't surface the header it sets internally.
    let referer = match resolved.referer.as_ref() {
        Some(r) if !r.is_empty() => Some(r.clone()),
        _ => match url::Url::parse(&resolved.selected_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
        {
            Some(h) if h.ends_with("fast4speed.rsvp") => Some("https://allmanga.to".to_string()),
            _ => None,
        },
    };

    let launch = LaunchArgs {
        stream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player,
        // TODO(green): plumb cfg.external_player_kind +
        // cfg.external_player_custom_args once those fields land.
        player_kind: external_player::ExternalPlayerKind::Mpv,
        custom_args_template: None,
    };
    external_player::open_external_player(&launch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anicli::parser::DebugOutput;

    /// `play()` and `play_external()` are thin wrappers around
    /// `run_debug` + the relevant terminal action; the integration
    /// test in `tests/api_play.rs` exercises the full flow against a
    /// real ani-cli with a curl shim. These unit tests pin the
    /// mapping from `DebugOutput` → `CreateSessionArgs` /
    /// `LaunchArgs` so a future refactor of the field names is loud.

    #[test]
    fn debug_output_with_referer_and_subtitle_maps_to_session_args() {
        let debug = DebugOutput {
            selected_url: "https://wixmp.example/video.mp4".into(),
            all_links: vec![],
            referer: Some("https://allmanga.to".into()),
            subtitle_url: Some("https://wixmp.example/subs.vtt".into()),
        };
        // Mirrors the conversion inside `play()`. Kept in sync via
        // the integration test; this asserts the field-by-field
        // mapping is intact.
        let session_args = CreateSessionArgs {
            upstream_url: debug.selected_url.clone(),
            referer: debug.referer.clone().unwrap_or_default(),
            subtitle_url: debug.subtitle_url.clone(),
        };
        assert_eq!(session_args.upstream_url, "https://wixmp.example/video.mp4");
        assert_eq!(session_args.referer, "https://allmanga.to");
        assert_eq!(
            session_args.subtitle_url.as_deref(),
            Some("https://wixmp.example/subs.vtt")
        );
    }

    #[test]
    fn debug_output_without_referer_maps_to_empty_referer_string() {
        // CreateSessionArgs.referer is a required `String` (not
        // Option). We map None → empty string; the proxy treats that
        // as "send no Referer header." This test pins that contract.
        let debug = DebugOutput {
            selected_url: "https://x/y.mp4".into(),
            all_links: vec![],
            referer: None,
            subtitle_url: None,
        };
        let session_args = CreateSessionArgs {
            upstream_url: debug.selected_url,
            referer: debug.referer.unwrap_or_default(),
            subtitle_url: debug.subtitle_url,
        };
        assert_eq!(session_args.referer, "");
        assert!(session_args.subtitle_url.is_none());
    }

    /// Build an `AppState` for the `try_serve_cached` tests. Mirrors
    /// `app::tests::fake_state` (private, unreachable from here) so the
    /// shape stays in lock-step.
    fn state_with_proxy_origin() -> AppState {
        use crate::app::SCRAPER_CONCURRENCY;
        use crate::meta::kitsu::KitsuClient;
        use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
        use std::sync::Arc;
        use tokio::sync::Semaphore;
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: std::path::PathBuf::from("/tmp/ani-cli"),
            history_path: std::path::PathBuf::from("/tmp/ani-cli/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path: std::path::PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
        }
    }

    /// Build a CachedResolution with the new show_id/show_title fields
    /// defaulted to empty (so try_serve_cached's history-write skip
    /// branch fires). Tests that want history-write coverage override
    /// the two fields explicitly.
    fn cached_blank(upstream_url: String, referer: String, kind: MediaKind) -> CachedResolution {
        CachedResolution {
            upstream_url,
            referer,
            subtitle_url: None,
            media_kind: kind,
            show_id: String::new(),
            show_title: String::new(),
        }
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_when_url_is_unparseable() {
        // A corrupt cache row with garbage in upstream_url shouldn't
        // crash — fall through to ani-cli.
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            "not://a valid url at all".into(),
            String::new(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_none());
    }

    #[tokio::test]
    async fn try_serve_cached_returns_session_on_2xx_head() {
        // Cache hit happy path: upstream HEAD returns 200 → we register
        // a session and return its CreateSessionResponse. This is the
        // ~50ms path that replaces the ~30s ani-cli spawn.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/video.mp4"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/video.mp4", server.uri()),
            String::new(),
            MediaKind::Mp4,
        );
        let resp = try_serve_cached(&state, &cached).await.expect("hit");
        // Session is freshly created, but the upstream + kind match.
        assert!(resp.media_url.contains("/file.mp4"));
        assert_eq!(resp.media_kind, MediaKind::Mp4);
        // The cache_hit flag is what tells the renderer whether a
        // player error is silently retryable. Cache-served responses
        // must set it; the post-ani-cli path must not.
        assert!(
            resp.cache_hit,
            "try_serve_cached must tag the response so the renderer can retry on player error"
        );
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_on_404() {
        // Stale wixmp URL — HEAD 404 means the row is dead. Return
        // None so the caller falls through to ani-cli (which will
        // overwrite the row with a fresh resolution).
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/expired.mp4", server.uri()),
            String::new(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_none());
    }

    #[tokio::test]
    async fn try_serve_cached_sends_referer_header_when_set() {
        // fast4speed.rsvp upstreams 403 without `Referer:
        // https://allmanga.to`. The cached referer must round-trip
        // through the HEAD validation; otherwise the row appears dead
        // even when it isn't.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/sub/1", server.uri()),
            "https://allmanga.to".into(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_some());
    }

    fn external_args(title: &str, episode: &str) -> PlayArgs {
        PlayArgs {
            title: title.into(),
            episode: episode.into(),
            mode: "sub".into(),
            quality: Some("best".into()),
            episode_count: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        }
    }

    fn external_cfg() -> crate::config::Config {
        crate::config::Config {
            external_player: "test-player".into(),
            ..Default::default()
        }
    }

    fn seed_play_cache(state: &AppState, args: &PlayArgs, upstream: &str, referer: &str) {
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            args.quality.as_deref().unwrap_or("best"),
            &args.episode,
        );
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: upstream.into(),
                referer: referer.into(),
                subtitle_url: None,
                media_kind: MediaKind::Mp4,
                show_id: "abc".into(),
                show_title: "Test (12 episodes)".into(),
            },
        );
    }

    /// Drive `play_with_progress` through the cache-hit short-circuit
    /// so the lines inside the `if let Some(cached) = ...` branch
    /// (history-write skip, info!, the early `return Ok(resp)`) all
    /// run. This is a real test of the embedded-player fast path —
    /// it would have caught the regression that prompted the
    /// long-term cache to ship.
    #[tokio::test]
    async fn play_with_progress_returns_cache_hit_response_when_head_succeeds() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Cached Show", "5");
        let upstream = format!("{}/cached.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let resp = play_with_progress(&state, &args, |_| {})
            .await
            .expect("cache-hit returns Ok");
        assert!(
            resp.cache_hit,
            "play_with_progress must tag cache-hit responses so the renderer can retry on player error"
        );
        assert_eq!(resp.media_kind, MediaKind::Mp4);
    }

    /// Same shape, but with a non-empty referer + show_id — exercises
    /// the cache-hit history-write branch (lines 266-282 in the file
    /// before this test landed). Without this the upsert-on-cache-hit
    /// path was uncovered, leaving Continue Watching's "I just played
    /// this" feedback silently broken if it regressed.
    #[tokio::test]
    async fn play_with_progress_writes_history_on_cache_hit_with_show_id() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Show With History", "3");
        let upstream = format!("{}/cached.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        // Non-prefetch click → history must be written. The upsert
        // target is state.history_path, which is /tmp/ani-cli/ani-hsts
        // by default — make it a real tempfile so the write
        // succeeds and we can assert against it.
        let td = tempfile::tempdir().expect("tempdir");
        let mut state = state;
        state.history_path = td.path().join("ani-hsts");
        let _ = play_with_progress(&state, &args, |_| {}).await.expect("ok");
        // The history file must exist with one row referencing the
        // seeded show_id.
        let body = std::fs::read_to_string(&state.history_path).unwrap_or_default();
        assert!(
            body.contains("abc"),
            "history must contain seeded show_id; got: {body:?}"
        );
    }

    /// HEAD failure → cache row evicted, function falls through to
    /// ani-cli (which fails because the spawn binary path is bogus
    /// in the test fixture). The test just needs to confirm the
    /// eviction-and-fallthrough branch runs without panicking;
    /// covers lines 288-292 (eviction warn).
    #[tokio::test]
    async fn play_with_progress_evicts_cache_when_head_fails_then_returns_error() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Stale Show", "1");
        let upstream = format!("{}/dead.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let r = play_with_progress(&state, &args, |_| {}).await;
        assert!(r.is_err(), "ani-cli fallback must error in the test env");
        // Cache row should be gone.
        let key = play_resolution_cache::cache_key(&args.title, &args.mode, "best", &args.episode);
        assert!(
            play_resolution_cache::get(&state.cache_pool, &key)
                .ok()
                .flatten()
                .is_none(),
            "stale row must be evicted on HEAD failure"
        );
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_none_on_cache_miss() {
        let state = state_with_proxy_origin();
        let args = external_args("Never Played", "1");
        let cfg = external_cfg();
        assert!(try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_launch_args_on_2xx_head() {
        // Happy path — cache hit + HEAD ok → caller can hand the
        // returned LaunchArgs to mpv without re-running ani-cli.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Naruto", "5");
        seed_play_cache(&state, &args, &format!("{}/v.mp4", server.uri()), "");
        let cfg = external_cfg();

        let launch = try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .expect("hit");

        assert!(launch.stream_url.contains("/v.mp4"));
        assert!(
            launch.referer.is_none(),
            "empty cached referer must round-trip as None"
        );
        assert_eq!(launch.player_command, "test-player");
        assert_eq!(launch.title.as_deref(), Some("Naruto · ep 5"));
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_evicts_and_returns_none_on_404() {
        // Stale upstream — HEAD 404. The cache row must be evicted so a
        // fresh ani-cli run will overwrite, AND we return None so the
        // caller falls through to the fresh path.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Stale", "1");
        let upstream = format!("{}/dead.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let cfg = external_cfg();

        let result = try_launch_args_from_cache(&state, &args, &cfg).await;
        assert!(result.is_none());

        // Cache row should be gone; a fresh attempt would re-resolve.
        let key = play_resolution_cache::cache_key(&args.title, &args.mode, "best", &args.episode);
        assert!(
            play_resolution_cache::get(&state.cache_pool, &key)
                .ok()
                .flatten()
                .is_none(),
            "stale cache row must be evicted on HEAD failure"
        );
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_round_trips_referer_and_subtitle() {
        // fast4speed.rsvp + signed-URL upstreams need the cached
        // Referer header forwarded; subtitle URL too (mpv consumes it).
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Fast4", "3");
        let key = play_resolution_cache::cache_key(&args.title, &args.mode, "best", &args.episode);
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: format!("{}/sub/3", server.uri()),
                referer: "https://allmanga.to".into(),
                subtitle_url: Some("https://example/cap.vtt".into()),
                media_kind: MediaKind::Mp4,
                show_id: "x".into(),
                show_title: "Fast4 (12 episodes)".into(),
            },
        );
        let cfg = external_cfg();

        let launch = try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .expect("hit");
        assert_eq!(launch.referer.as_deref(), Some("https://allmanga.to"));
        assert_eq!(
            launch.subtitle_url.as_deref(),
            Some("https://example/cap.vtt")
        );
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_none_on_unparseable_url() {
        let state = state_with_proxy_origin();
        let args = external_args("Bad URL", "1");
        seed_play_cache(&state, &args, "not://a valid url", "");
        let cfg = external_cfg();
        assert!(try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .is_none());
    }

    #[test]
    fn play_args_quality_defaults_to_best() {
        let args = PlayArgs {
            title: "test".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            episode_count: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        assert_eq!(args.quality.as_deref().unwrap_or("best"), "best");
    }

    #[test]
    fn play_args_alt_titles_default_to_empty_when_omitted() {
        // Older clients (and `/api/play/external` callers that don't
        // know about the field yet) send the JSON without alt_titles.
        // Serde default keeps that path working — the play flow still
        // runs with just the canonical title.
        let json = r#"{"title":"x","episode":"1","mode":"sub"}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    #[test]
    fn play_args_deserializes_alt_titles_when_present() {
        let json = r#"{"title":"JoJo's Bizarre Adventure: Stone Ocean","episode":"1","mode":"sub","alt_titles":["Jojo no Kimyou na Bouken Part 6: Stone Ocean","ジョジョの奇妙な冒険 ストーンオーシャン"]}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert_eq!(args.alt_titles.len(), 2);
        assert_eq!(
            args.alt_titles[0],
            "Jojo no Kimyou na Bouken Part 6: Stone Ocean"
        );
    }

    #[test]
    fn play_args_deserializes_alt_titles_from_newline_joined_query_string() {
        // SSE GET path — EventSource can't POST, and serde_urlencoded
        // can't deserialize Vec<String> from repeated keys. The frontend
        // joins alt_titles with `\n` for this path; backend splits.
        let qs = "title=Stone+Ocean&episode=1&mode=sub&alt_titles=a%0Ab%0Ac";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert_eq!(args.alt_titles, vec!["a", "b", "c"]);
    }

    #[test]
    fn play_args_treats_empty_alt_titles_string_as_empty_vec() {
        // The frontend sends `alt_titles=` for shows whose Kitsu titles
        // map is empty (rare but real). Backend must still parse.
        let qs = "title=X&episode=1&mode=sub&alt_titles=";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    /// Pass a literal `null` for alt_titles so the deserializer's
    /// `None` arm fires (serde's `default` only short-circuits when
    /// the FIELD is missing; an explicit `null` still goes through
    /// `deserialize_alt_titles`).
    #[test]
    fn play_args_treats_explicit_null_alt_titles_as_empty_vec() {
        let json = r#"{"title":"x","episode":"1","mode":"sub","alt_titles":null}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    /// `prefetch` tolerates the JSON bool form, the SSE-string form
    /// ("1" / "true" / "yes"), and missing / null. Test all three
    /// truthy strings + the negative ones + null so the
    /// `deserialize_loose_bool` switch is fully exercised.
    #[test]
    fn play_args_loose_bool_accepts_true_strings() {
        for truthy in ["1", "true", "yes"] {
            let qs = format!("title=X&episode=1&mode=sub&prefetch={truthy}");
            let args: PlayArgs = serde_urlencoded::from_str(&qs).expect("parses");
            assert!(args.prefetch, "expected prefetch=true for {truthy:?}");
        }
    }

    #[test]
    fn play_args_loose_bool_treats_other_strings_as_false() {
        for falsy in ["0", "false", "no", "wat"] {
            let qs = format!("title=X&episode=1&mode=sub&prefetch={falsy}");
            let args: PlayArgs = serde_urlencoded::from_str(&qs).expect("parses");
            assert!(!args.prefetch, "expected prefetch=false for {falsy:?}");
        }
    }

    #[test]
    fn play_args_loose_bool_accepts_explicit_json_bool() {
        // Direct POST clients still send the field as a JSON
        // boolean — serde_json's untagged enum tries the Bool arm
        // first.
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":true}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_loose_bool_treats_explicit_null_as_false() {
        // Pin the None-arm of `deserialize_loose_bool` — explicit
        // `null` should keep the click-path default rather than
        // erroring.
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":null}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(!args.prefetch);
    }

    #[test]
    fn play_args_prefetch_defaults_to_false_when_omitted() {
        // Older clients (and click handlers that don't bother passing
        // the field) leave prefetch implicit — must default to false
        // so the history-write path stays active for clicks.
        let json = r#"{"title":"x","episode":"1","mode":"sub"}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(!args.prefetch);
    }

    #[test]
    fn play_args_prefetch_accepts_json_bool() {
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":true}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_prefetch_accepts_query_string_one() {
        // SSE GET path: serde_urlencoded can't decode bool directly.
        // The custom deserializer handles "1" / "true" / "yes" / "0".
        let qs = "title=X&episode=1&mode=sub&prefetch=1";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_prefetch_zero_string_means_false() {
        let qs = "title=X&episode=1&mode=sub&prefetch=0";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(!args.prefetch);
    }

    /// Build a Candidate row with the right `availableEpisodes.sub`
    /// field for the helper-selection tests below. The full struct
    /// is verbose; this keeps each test focused on the behaviour it's
    /// asserting (which title wins, which candidate index ani-cli
    /// gets).
    fn cand(id: &str, name: &str, sub_eps: u32) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            available_episodes: crate::scraper::allanime::AvailableEpisodes {
                sub: sub_eps,
                dub: 0,
            },
        }
    }

    #[test]
    fn select_first_with_hits_walks_alt_titles_when_episode_count_unknown() {
        // Real-world reproducer: Kitsu returns null `episodeCount` for
        // some shows even when they're finished (Stone Ocean Part 6 was
        // observed in the wild). The early `let Some(expected) = …`
        // guard used to short-circuit the alt_titles loop, which meant
        // `args.title` was the only thing tried — and for shows whose
        // canonical doesn't match allmanga's index, that's a guaranteed
        // miss. The fix: always walk the candidate list, only the
        // pick_by_ep_count step is gated on episode_count.
        let results = vec![
            ("JoJo's Bizarre Adventure: Stone Ocean".into(), vec![]),
            (
                "Jojo no Kimyou na Bouken Part 6: Stone Ocean".into(),
                vec![cand("a1", "Stone Ocean", 12)],
            ),
        ];
        // expected = None signals "no Kitsu episode_count to disambiguate".
        let (title, idx) = select_first_with_hits_opt(
            "JoJo's Bizarre Adventure: Stone Ocean",
            &results,
            None,
            "sub",
        );
        assert_eq!(title, "Jojo no Kimyou na Bouken Part 6: Stone Ocean");
        assert_eq!(idx, 1, "first hit when no ep_count to compare");
    }

    #[test]
    fn select_first_with_hits_returns_primary_when_every_list_is_empty() {
        // Stone Ocean reproduces this when every candidate title (canonical
        // English + en_jp + ja_jp) misses allmanga's index. We fall
        // through to the primary so the play flow's downstream error
        // surfaces a real "no upstream" rather than silently picking
        // index 1 of nothing.
        let results: Vec<(String, Vec<Candidate>)> =
            vec![("primary".into(), vec![]), ("alt1".into(), vec![])];
        let (title, idx) = select_first_with_hits("primary", &results, 38, "sub");
        assert_eq!(title, "primary");
        assert_eq!(idx, 1);
    }

    #[test]
    fn select_first_with_hits_uses_first_non_empty_list() {
        // Primary has hits — we never even look at the alt titles.
        let results = vec![
            ("primary".into(), vec![cand("p1", "Primary Show", 38)]),
            ("alt1".into(), vec![cand("a1", "Alt Show", 12)]),
        ];
        let (title, idx) = select_first_with_hits("primary", &results, 38, "sub");
        assert_eq!(title, "primary");
        assert_eq!(idx, 1, "single-candidate list always picks index 1");
    }

    #[test]
    fn select_first_with_hits_skips_empty_primary_to_alt_with_hits() {
        // Stone Ocean Part 6 case: canonical English → 0 hits, en_jp
        // → multiple hits. We must use en_jp.
        let results = vec![
            ("JoJo's Bizarre Adventure: Stone Ocean".into(), vec![]),
            (
                "Jojo no Kimyou na Bouken Part 6: Stone Ocean".into(),
                vec![
                    cand("a1", "Stone Ocean main", 38),
                    cand("a2", "side story", 1),
                ],
            ),
        ];
        let (title, idx) =
            select_first_with_hits("JoJo's Bizarre Adventure: Stone Ocean", &results, 38, "sub");
        assert_eq!(title, "Jojo no Kimyou na Bouken Part 6: Stone Ocean");
        // 38-ep candidate is index 1 (closer to expected 38 than the
        // 1-ep side story).
        assert_eq!(idx, 1);
    }

    #[test]
    fn select_first_with_hits_picks_by_ep_count_within_chosen_list() {
        // Naruto: Shippuden case — multiple candidates under one title;
        // the disambiguator chooses by episode count.
        let results = vec![(
            "Naruto: Shippuden".into(),
            vec![
                cand("a1", "side story", 1),
                cand("a2", "main shippuden", 500),
            ],
        )];
        let (title, idx) = select_first_with_hits("Naruto: Shippuden", &results, 500, "sub");
        assert_eq!(title, "Naruto: Shippuden");
        // Index 2 = the 500-ep main show.
        assert_eq!(idx, 2);
    }
}
