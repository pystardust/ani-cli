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
    for (title, cands) in results {
        if cands.is_empty() {
            continue;
        }
        // pick_by_ep_count returns None only on empty input; we already
        // filtered that, so the unwrap_or is a belt-and-braces fallback.
        let pick = scraper::pick_by_ep_count(cands, expected, mode).unwrap_or(1);
        return (title.clone(), pick);
    }
    (primary.to_string(), 1)
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
async fn pick_title_and_index(state: &AppState, args: &PlayArgs) -> (String, usize) {
    let primary = args.title.clone();
    let Some(expected) = args.episode_count else {
        return (primary, 1);
    };
    let mode = if args.mode == "dub" { "dub" } else { "sub" };

    // Build (title, candidates) pairs by querying allanime for each
    // candidate title in turn. Stop at the first non-empty list — no
    // point making three GraphQL calls when the canonical worked.
    let mut results: Vec<(String, Vec<Candidate>)> = Vec::new();
    for title in
        std::iter::once(args.title.as_str()).chain(args.alt_titles.iter().map(String::as_str))
    {
        match scraper::search(&state.proxy_http, title, mode, None).await {
            Ok(cands) => {
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

    let (chosen_title, pick) = select_first_with_hits(&primary, &results, expected, mode);
    tracing::info!(
        primary = %primary,
        alt_count = args.alt_titles.len(),
        chosen_title = %chosen_title,
        expected_eps = expected,
        pick = pick,
        "play: disambiguated allanime candidate by episode count",
    );
    (chosen_title, pick)
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

    // Pick which (title, candidate index) ani-cli should use. The title
    // may differ from args.title when alt_titles produced the winning
    // hit (e.g. romanized fallback for shows whose Kitsu canonicalTitle
    // is the English form). See pick_title_and_index().
    let (search_title, select_index) = pick_title_and_index(state, args).await;

    let resolved = run_debug_streaming(
        &opts,
        &search_title,
        &args.episode,
        quality,
        &args.mode,
        select_index,
        |line| {
            if let Some(p) = parse_progress_line(line) {
                on_progress(p);
            }
        },
    )
    .await?;

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

    let session_args = CreateSessionArgs {
        upstream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
    };
    create_session_with_kind(state, &session_args, kind)
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
    let (search_title, select_index) = pick_title_and_index(state, args).await;
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
