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
    #[serde(default, deserialize_with = "deserialize_alt_titles")]
    pub alt_titles: Vec<String>,
}

/// Accept either a JSON array of strings or a single newline-joined
/// string for `alt_titles`. The string form is the SSE-query path —
/// serde_urlencoded can't decode `alt_titles=a&alt_titles=b` as a Vec.
fn deserialize_alt_titles<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        List(Vec<String>),
        Joined(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => Vec::new(),
        Some(Wire::List(v)) => v,
        Some(Wire::Joined(s)) => s
            .split('\n')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect(),
    })
}

/// Choose which `(title, candidate_index)` to feed `ani-cli -S`. Walks
/// the supplied `(title, candidates)` results in order and returns
/// the first one whose candidate list is non-empty, paired with the
/// 1-based index from [`scraper::pick_by_ep_count`] (closest match by
/// episode count to `expected`).
///
/// When every list is empty (or the slice is empty), returns
/// `(primary, 1)` — the legacy `-S 1` behaviour callers used before
/// disambiguation existed. The play flow falls through to ani-cli
/// with the primary title; ani-cli's own search will likely fail too,
/// but the user sees a real error instead of a fake "we picked
/// candidate 1" silent miss.
#[must_use]
pub fn select_first_with_hits(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: u32,
    mode: &str,
) -> (String, usize) {
    select_first_with_hits_opt(primary, results, Some(expected), mode)
}

/// `select_first_with_hits` variant where `expected` may be unknown.
/// When `expected` is `None`, returns the first non-empty list with
/// candidate index 1 (allanime's own ranking — same as ani-cli's
/// default `-S 1`). When `Some`, behaves identically to the v1 helper.
///
/// This is the path Stone Ocean Part 6 needs: Kitsu's episode_count
/// is null for that entry, so we can't disambiguate by ep count, but
/// we still need to walk alt_titles to find one allmanga indexes.
#[must_use]
pub fn select_first_with_hits_opt(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: Option<u32>,
    mode: &str,
) -> (String, usize) {
    let (title, idx, _) = select_first_with_hits_with_candidate(primary, results, expected, mode);
    (title, idx)
}

/// Like [`select_first_with_hits_opt`] but also returns a clone of the
/// chosen [`Candidate`] (the row whose `id` + `name` we'll cache for
/// the history-write feedback path). `None` for the candidate when no
/// list had hits — the caller falls back to writing nothing.
#[must_use]
pub fn select_first_with_hits_with_candidate(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: Option<u32>,
    mode: &str,
) -> (String, usize, Option<Candidate>) {
    for (title, cands) in results {
        if cands.is_empty() {
            continue;
        }
        let pick = match expected {
            Some(n) => scraper::pick_by_ep_count(cands, n, mode).unwrap_or(1),
            None => 1,
        };
        // `pick` is 1-based; clamp into the slice in case
        // pick_by_ep_count ever returns out-of-bounds (defence in
        // depth — its current contract is 1..=len).
        let idx0 = pick.saturating_sub(1).min(cands.len() - 1);
        return (title.clone(), pick, Some(cands[idx0].clone()));
    }
    (primary.to_string(), 1, None)
}

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
async fn pick_title_and_index(
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

fn debug_options_for(state: &AppState) -> DebugOptions {
    DebugOptions {
        ani_cli_path: state.ani_cli_path.clone(),
        // ani-cli writes/reads its history file alongside the GUI's,
        // so plays through here also surface in Continue Watching.
        hist_dir: state
            .history_path
            .parent()
            .map(std::path::Path::to_path_buf),
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
    let opts = debug_options_for(state);
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
            // for legacy rows (show_id empty) — they re-cache fresh on
            // their next miss and pick up the metadata then.
            if !cached.show_id.is_empty() {
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
    })?;

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

/// HEAD-validate a cached upstream URL. Returns a fresh
/// CreateSessionResponse on success, or `None` if the URL is dead /
/// unreachable / returns an error status — caller should fall through
/// to a fresh ani-cli spawn.
async fn try_serve_cached(
    state: &AppState,
    cached: &CachedResolution,
) -> Option<CreateSessionResponse> {
    let url = url::Url::parse(&cached.upstream_url).ok()?;
    let mut req = state.proxy_http.head(url.as_str());
    if !cached.referer.is_empty() {
        req = req.header(reqwest::header::REFERER, &cached.referer);
    }
    let resp = req.send().await.ok()?;
    // 2xx and 3xx (CDN edge redirects) both count as live. 4xx/5xx
    // mean the URL has rotated or the CDN is unhappy — treat as dead
    // and let ani-cli resolve fresh.
    if !(resp.status().is_success() || resp.status().is_redirection()) {
        return None;
    }
    let session_args = CreateSessionArgs {
        upstream_url: cached.upstream_url.clone(),
        referer: cached.referer.clone(),
        subtitle_url: cached.subtitle_url.clone(),
    };
    let mut resp = create_session_with_kind(state, &session_args, cached.media_kind).ok()?;
    // Tag so the renderer can decide whether a player error is
    // retryable (cache hit can be evicted + re-resolved) or terminal
    // (fresh fetch, no cache to clear).
    resp.cache_hit = true;
    Some(resp)
}

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
    let opts = debug_options_for(state);
    let quality = args.quality.as_deref().unwrap_or("best");
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

    // Player command comes from the user's settings file with the
    // documented default (`mpv`). Falling back to the default config
    // is intentional: a corrupt settings file shouldn't prevent the
    // user from launching mpv if it's on PATH.
    let cfg = read_config(&state.config_path).unwrap_or_default();

    let launch = LaunchArgs {
        stream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player,
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

    #[test]
    fn play_args_quality_defaults_to_best() {
        let args = PlayArgs {
            title: "test".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            episode_count: None,
            alt_titles: vec![],
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
