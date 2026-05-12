//! `open_syncplay` command — launches the user's locally-installed
//! [Syncplay](https://syncplay.pl) binary on a resolved stream URL.
//!
//! Syncplay is a third-party PyQt5 application that wraps the user's
//! own mpv (or vlc/iina) and connects to a Syncplay server to keep
//! room members' playback in sync. ani-gui hands it the resolved
//! upstream URL as the positional file argument; Syncplay handles
//! everything else (room dialog, server connection, wrapped-player
//! flags) in its own UI.
//!
//! Player-kind flag mapping past `--` mirrors what
//! `external_player::build_argv` emits for the direct-spawn path,
//! so a user who configured `external_player_kind = Vlc` gets the
//! same `--http-referrer=` flag whether they click "Open in
//! external" or "Watch together". Custom kind bypasses the
//! forwarding — the wrapped player's own config carries the
//! per-stream args.
//!
//! Bundling is intentionally out of scope — Syncplay is a heavyweight
//! PyQt5 app and `apt install syncplay` is broken on Ubuntu 24.04
//! (Noble ships 1.7.0 which crashes on Python 3.12). When the
//! configured binary can't be spawned, the frontend surfaces an
//! `ErrorOverlay` modal with a link to syncplay.pl — same pattern as
//! the ffmpeg-missing dialog.

use serde::Deserialize;

use crate::commands::external_player::ExternalPlayerKind;
use crate::error::{AniError, Result};

/// Arguments to the command. Caller supplies the resolved stream URL
/// (the play flow's same URL the embedded player would consume), the
/// configured Syncplay binary path, and an optional `Referer:` value
/// that gets forwarded to Syncplay's wrapped mpv. Frontend reads the
/// binary from `Config::syncplay_binary`; the referer is inferred by
/// `play_syncplay` from the resolved upstream (mirrors the
/// `play_external` path's fast4speed.rsvp → allmanga.to fallback).
#[derive(Debug, Deserialize)]
pub struct SyncplayLaunchArgs {
    /// The resolved stream URL (mp4 or m3u8). Syncplay's positional
    /// "file" argument.
    pub stream_url: String,
    /// Syncplay binary path. Resolved from `Config::syncplay_binary`
    /// (per-OS default; user-overridable in settings).
    pub binary: String,
    /// Optional `Referer:` header value the upstream CDN requires.
    /// Forwarded to Syncplay's wrapped player via the mpv-style
    /// `--referrer=` flag after the `--` separator. fast4speed.rsvp
    /// 403s without `Referer: https://allmanga.to`, so the same
    /// inference logic `play_external` uses applies to Syncplay too.
    /// Old payloads without this field decode as `None`.
    #[serde(default)]
    pub referer: Option<String>,
    /// Optional sidecar subtitle URL (`.vtt`) when ani-cli surfaces a
    /// soft-subtitle track separately from the stream. Forwarded to
    /// the wrapped player via the kind-appropriate `--sub-file=` /
    /// equivalent flag. Without this, Syncplay's wrapped player
    /// opens the video but drops the subtitles even though the
    /// embedded and external-player paths show them. Old payloads
    /// without this field decode as `None`.
    #[serde(default)]
    pub subtitle_url: Option<String>,
    /// Which media player Syncplay wraps. Drives the flag syntax
    /// emitted past `--`: mpv takes `--referrer=`, VLC takes
    /// `--http-referrer=`, IINA takes `--mpv-referrer=` (the
    /// passthrough to its embedded mpv). `Custom` emits no
    /// player-specific flags — the wrapped player's own config
    /// carries them, same escape hatch the external-player path's
    /// Custom kind uses.
    ///
    /// Reuses `Config::external_player_kind` directly: most users
    /// have one media player installed and Syncplay defaults to
    /// wrapping the same one they picked for "Open in external".
    /// Old payloads without this field decode as `Mpv` (the
    /// upstream Syncplay default).
    #[serde(default)]
    pub player_kind: ExternalPlayerKind,
    /// Path to the media-player binary Syncplay should wrap.
    /// Forwarded via Syncplay's `--player-path=` flag, which
    /// overrides Syncplay's own `syncplay.ini` setting. Pinning the
    /// wrapped binary here means `player_kind` is guaranteed to
    /// match what Syncplay actually launches — no more "ani-gui says
    /// mpv but Syncplay's .ini was last set to VLC" mismatches.
    /// Empty / missing skips the flag (defers to Syncplay's own
    /// config, same back-compat behavior pre-PR).
    #[serde(default)]
    pub player_binary: String,
}

/// Build the argv that would be passed to `Command::new(binary).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// Syncplay's CLI grammar is `syncplay [options] [file] -- [player
/// options]`. The `--` separator forwards everything after it to the
/// wrapped player. We pick the referer flag based on `player_kind`
/// so Syncplay→VLC gets VLC's `--http-referrer=` instead of mpv's
/// `--referrer=`. `--sub-file=` is the same flag on mpv, VLC, and
/// IINA, so no branching needed for subtitle. Custom kind emits no
/// player-specific flags — the wrapped player's own config carries
/// referer / sub-file, same escape hatch the external-player path's
/// Custom kind uses.
#[must_use]
pub fn build_argv(args: &SyncplayLaunchArgs) -> Vec<String> {
    let mut argv = Vec::new();
    // `--player-path=` is a Syncplay option (NOT a player option),
    // so it goes before the file/positional and before any `--`.
    // Syncplay's CLI args take precedence over its .ini, and its
    // player classes resolve bare PATH commands ("mpv", "vlc")
    // via `getExpandedPath`'s `os.environ['PATH']` walk, so
    // forwarding the value verbatim keeps player_kind in lockstep
    // with what Syncplay actually launches — for absolute paths
    // and for the default bare-command setups alike.
    let player_binary = args.player_binary.trim();
    if !player_binary.is_empty() {
        argv.push(format!("--player-path={player_binary}"));
    }
    argv.push(args.stream_url.clone());
    if matches!(args.player_kind, ExternalPlayerKind::Custom) {
        // Custom: defer all per-stream args to the wrapped player's
        // own config. Forwarding anything risks an "unknown option"
        // complaint from a player whose flag shape we don't know.
        return argv;
    }
    let referrer_flag = match args.player_kind {
        ExternalPlayerKind::Mpv => "--referrer=",
        ExternalPlayerKind::Vlc => "--http-referrer=",
        ExternalPlayerKind::Iina => "--mpv-referrer=",
        // Custom is short-circuited above; this arm is unreachable
        // but the compiler can't see that without the explicit
        // matches!() above, so keep the explicit arm.
        ExternalPlayerKind::Custom => "--referrer=",
    };
    let referer = args.referer.as_deref().filter(|s| !s.is_empty());
    let subtitle = args.subtitle_url.as_deref().filter(|s| !s.is_empty());
    if referer.is_some() || subtitle.is_some() {
        argv.push("--".to_string());
        if let Some(r) = referer {
            argv.push(format!("{referrer_flag}{r}"));
        }
        if let Some(s) = subtitle {
            argv.push(format!("--sub-file={s}"));
        }
    }
    argv
}

/// Launch the configured Syncplay binary against the resolved stream
/// URL. Returns once the spawn completes; the child is detached the
/// same way external_player.rs detaches the user's mpv.
///
/// # Errors
/// - [`AniError::SyncplaySpawnFailed`] if the configured binary can't
///   be spawned (not on PATH, doesn't exist, or path doesn't point at
///   an executable). Carries the binary string so the UI can name
///   the failed command in the error dialog and link the user to
///   <https://syncplay.pl/download/>.
pub fn open_syncplay(args: &SyncplayLaunchArgs) -> Result<()> {
    if args.binary.trim().is_empty() {
        return Err(AniError::SyncplaySpawnFailed {
            binary: args.binary.clone(),
        });
    }
    let argv = build_argv(args);
    let mut cmd = std::process::Command::new(&args.binary);
    cmd.args(&argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|_| AniError::SyncplaySpawnFailed {
            binary: args.binary.clone(),
        })
}

#[cfg(test)]
#[path = "syncplay_test.rs"]
mod tests;
