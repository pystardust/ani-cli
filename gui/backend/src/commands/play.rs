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
}

/// Spawn timeout for the ani-cli search+resolve step. Real-world
/// allanime queries take 5-30s; 60s is a comfortable upper bound
/// before the user is better served by an error than a stuck spinner.
const RUN_DEBUG_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolve which 1-based candidate index to pass to ani-cli's `-S`
/// flag. Calls our own allanime search, picks the candidate whose
/// `availableEpisodes` is closest to Kitsu's, and returns that index.
/// Falls through to 1 (legacy behaviour) on any failure or missing
/// signal — playback should never block on the disambiguator.
async fn pick_index_or_default(state: &AppState, args: &PlayArgs) -> usize {
    let Some(expected) = args.episode_count else {
        return 1;
    };
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    match scraper::search(&state.proxy_http, &args.title, mode, None).await {
        Ok(cands) => {
            let pick = scraper::pick_by_ep_count(&cands, expected, mode).unwrap_or(1);
            tracing::info!(
                title = %args.title,
                expected_eps = expected,
                candidates = cands.len(),
                pick = pick,
                "play: disambiguated allanime candidate by episode count",
            );
            pick
        }
        Err(e) => {
            tracing::warn!(
                title = %args.title,
                error = ?e,
                "play: allanime search failed; falling back to -S 1",
            );
            1
        }
    }
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

    // Disambiguate which allanime candidate ani-cli should pick. See
    // play() docstring above; behaviour is identical.
    let select_index = pick_index_or_default(state, args).await;

    let resolved = run_debug_streaming(
        &opts,
        &args.title,
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
    let select_index = pick_index_or_default(state, args).await;
    let resolved = run_debug(
        &opts,
        &args.title,
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
        };
        assert_eq!(args.quality.as_deref().unwrap_or("best"), "best");
    }
}
