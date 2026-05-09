//! Download an episode via `ani-cli -d`. Mirrors the play command's
//! shape (same disambiguation, same Kitsu-driven select-index logic),
//! but instead of registering a stream session it spawns yt-dlp /
//! ffmpeg / aria2c via ani-cli to write an mp4 to disk.
//!
//! Progress lines (aria2c / yt-dlp / ffmpeg stderr) are forwarded to
//! the SSE handler in `api::get_download_stream` so the renderer can
//! show a live progress bar in the dock.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::anicli::process::{spawn_download, DownloadRequest};
use crate::app::AppState;
use crate::commands::play::{debug_options_for, pick_title_and_index, PlayArgs};
use crate::error::{AniError, Result};

/// Wire payload for the download endpoint. A near-clone of [`PlayArgs`]
/// (so the renderer can pass through the same metadata it gathered for
/// /api/play), with one extra field: an explicit destination directory
/// from the folder picker. When `None`, the resolver falls back to
/// `paths::download_dir()`.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadArgs {
    /// Canonical Kitsu title (drives the ani-cli search step).
    pub title: String,
    /// Episode number, as a string (matches the CLI's `-e <n>` shape).
    pub episode: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// `"best"` / `"worst"` / `"1080"` / etc. Defaults to `"best"`.
    #[serde(default)]
    pub quality: Option<String>,
    /// Kitsu's authoritative episode count — feeds the same
    /// disambiguator the play path uses.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Fallback titles tried when the canonical title returns no
    /// allanime hits. Same wire forms as [`PlayArgs::alt_titles`].
    #[serde(default, deserialize_with = "deserialize_alt_titles")]
    pub alt_titles: Vec<String>,
    /// Kitsu id of the show being downloaded; logged for traceability.
    #[serde(default)]
    pub kitsu_id: Option<String>,
    /// Absolute path to the directory the download lands in. The
    /// frontend's confirmation modal opens on `paths::download_dir()`
    /// and lets the user pick a different folder; the chosen path
    /// arrives here. `None` triggers the same default-resolution
    /// chain on the backend.
    #[serde(default)]
    pub download_dir: Option<String>,
}

/// SSE event body for each progress line forwarded from the downloader
/// (aria2c / yt-dlp / ffmpeg). Frontend renders the latest line under
/// each active download row.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// Raw stderr line from aria2c / yt-dlp / ffmpeg, ANSI-stripped.
    pub line: String,
}

/// SSE final-event body. `dest_dir` is the directory the file landed
/// in (so the renderer can fire a "reveal in folder" intent without
/// guessing the exact filename — ani-cli's name templating depends on
/// the upstream's `allanime_title`, which we don't surface here).
#[derive(Debug, Clone, Serialize)]
pub struct DownloadResponse {
    /// Directory the file was written to. Renderer feeds this to
    /// `revealInFolder` for the completion toast.
    pub dest_dir: String,
}

/// Drive a download from `args`. Picks the same (title, candidate
/// index) pair as the equivalent /api/play call so what was watched is
/// what gets saved, then spawns ani-cli with `-d` + the chosen
/// destination directory. `on_progress` is invoked for every stderr
/// line ani-cli forwards (aria2c progress, yt-dlp fragment events,
/// etc.).
///
/// # Errors
/// - [`AniError::Config`] when no destination is supplied and the
///   default resolver returns `None` (no `$XDG_DOWNLOAD_DIR`, no
///   `$HOME` — the renderer should always pass an explicit dir).
/// - [`AniError::Io`] if the destination directory can't be created.
/// - Otherwise propagates from [`spawn_download`].
pub async fn download_with_progress<F>(
    state: &AppState,
    args: &DownloadArgs,
    mut on_progress: F,
) -> Result<DownloadResponse>
where
    F: FnMut(DownloadProgress) + Send,
{
    let dest = resolve_dest(args)?;
    std::fs::create_dir_all(&dest).map_err(|_| AniError::Io)?;

    // Downloads run aria2c / yt-dlp / ffmpeg, which take minutes —
    // the play path's 60s wall-clock timeout would kill the download
    // mid-stream right after ani-cli finished link discovery (the
    // tools also keep stderr quiet during transfer, so we'd see no
    // progress to inform a longer timeout). Cap at one hour; user
    // can abort via the dock's Cancel button anyway.
    let mut opts = debug_options_for(state, None);
    opts.timeout = std::time::Duration::from_secs(60 * 60);
    let quality = args.quality.as_deref().unwrap_or("best");

    // Reuse play's disambiguator so a download started from the player
    // grabs the same allanime show ani-cli would have streamed.
    let play_view = play_args_view(args);
    let (search_title, select_index, _chosen) = pick_title_and_index(state, &play_view).await;

    tracing::info!(
        search_title = %search_title,
        episode = %args.episode,
        select_index = select_index,
        mode = %args.mode,
        quality = quality,
        dest = %dest.display(),
        "download: spawning ani-cli -d",
    );

    spawn_download(
        &opts,
        &DownloadRequest {
            query: &search_title,
            episode: &args.episode,
            quality,
            mode: &args.mode,
            select_index,
        },
        &dest,
        |line| {
            tracing::info!(line = %line, "anicli.dl.stderr");
            on_progress(DownloadProgress {
                line: line.to_string(),
            });
        },
    )
    .await?;

    Ok(DownloadResponse {
        dest_dir: dest.to_string_lossy().into_owned(),
    })
}

/// Resolve the destination directory from args + paths::download_dir.
/// Errors when neither path is available (no XDG, no HOME, no
/// override) — that case is unreachable from the GUI, which always
/// passes a value from the modal.
fn resolve_dest(args: &DownloadArgs) -> Result<PathBuf> {
    if let Some(s) = args.download_dir.as_deref().filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(s));
    }
    crate::config::paths::download_dir().ok_or(AniError::Config)
}

/// Project the download-only fields onto a synthetic [`PlayArgs`] so
/// the existing `pick_title_and_index` helper works unchanged. The
/// download path doesn't need the `prefetch` field — ani-cli's
/// download mode never touches `ani-hsts` (line 385 of upstream).
fn play_args_view(args: &DownloadArgs) -> PlayArgs {
    PlayArgs {
        title: args.title.clone(),
        episode: args.episode.clone(),
        mode: args.mode.clone(),
        quality: args.quality.clone(),
        episode_count: args.episode_count,
        alt_titles: args.alt_titles.clone(),
        prefetch: false,
        kitsu_id: args.kitsu_id.clone(),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_args_round_trips_through_json_with_optional_fields() {
        // Wire shape mirror — same fields the renderer sends. Quality,
        // episode_count, alt_titles, kitsu_id, download_dir are all
        // optional; the modal only requires title + episode + mode.
        let body = serde_json::json!({
            "title": "Naruto Shippuuden",
            "episode": "5",
            "mode": "sub",
            "quality": "1080",
            "alt_titles": ["NARUTO -ナルト- 疾風伝"],
            "download_dir": "/tmp/dl",
        });
        let parsed: DownloadArgs = serde_json::from_value(body).expect("parse");
        assert_eq!(parsed.title, "Naruto Shippuuden");
        assert_eq!(parsed.episode, "5");
        assert_eq!(parsed.mode, "sub");
        assert_eq!(parsed.quality.as_deref(), Some("1080"));
        assert_eq!(
            parsed.alt_titles,
            vec!["NARUTO -ナルト- 疾風伝".to_string()]
        );
        assert_eq!(parsed.download_dir.as_deref(), Some("/tmp/dl"));
    }

    #[test]
    fn download_args_alt_titles_accepts_newline_joined_string_for_sse_query_path() {
        // serde_urlencoded (the SSE GET path) can't decode repeated
        // ?alt_titles=a&alt_titles=b as a Vec, so the renderer joins
        // with \n. Same trick PlayArgs uses.
        let body = serde_json::json!({
            "title": "x",
            "episode": "1",
            "mode": "sub",
            "alt_titles": "a\nb\nc",
        });
        let parsed: DownloadArgs = serde_json::from_value(body).expect("parse");
        assert_eq!(
            parsed.alt_titles,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn resolve_dest_prefers_explicit_args_over_paths_helper() {
        let a = DownloadArgs {
            title: "x".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            episode_count: None,
            alt_titles: vec![],
            kitsu_id: None,
            download_dir: Some("/tmp/explicit".into()),
        };
        let p = resolve_dest(&a).expect("ok");
        assert_eq!(p, PathBuf::from("/tmp/explicit"));
    }
}
