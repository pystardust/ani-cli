//! `open_external_player` command — escape hatch that launches the
//! user's chosen external media player (default `mpv`) with the same
//! `--referer` and `--sub-file` flags `ani-cli` passes today.
//!
//! This is never an automatic fallback — it's user-triggered (a button
//! on the in-window player chrome). Auto-fallback would be confusing.

use serde::Deserialize;

use crate::error::{AniError, Result};

/// Arguments to the command. Frontend supplies the resolved stream URL +
/// optional referer + optional subtitle. The player command itself comes
/// from the user's config (default `mpv`).
#[derive(Debug, Deserialize)]
pub struct LaunchArgs {
    /// The resolved stream URL (mp4 or m3u8).
    pub stream_url: String,
    /// Optional `Referer:` value the upstream CDN requires.
    pub referer: Option<String>,
    /// Optional subtitle URL (`.vtt`).
    pub subtitle_url: Option<String>,
    /// Title shown in the player window's titlebar.
    pub title: Option<String>,
    /// Player command, e.g. `"mpv"`. Caller resolves this from settings.
    pub player_command: String,
}

/// Build the argv that would be passed to `Command::new(player).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// The argv order matches what `ani-cli`'s `play_episode` constructs in
/// the mpv branch (lines 394-402 of the script):
///     mpv [skip_flag] --force-media-title="..." [subs_flag] [refr_flag] <url>
#[must_use]
pub fn build_argv(args: &LaunchArgs) -> Vec<String> {
    let mut argv = Vec::with_capacity(8);
    if let Some(t) = &args.title {
        argv.push(format!("--force-media-title={t}"));
    }
    if let Some(s) = &args.subtitle_url {
        argv.push(format!("--sub-file={s}"));
    }
    if let Some(r) = &args.referer {
        argv.push(format!("--referrer={r}"));
    }
    argv.push(args.stream_url.clone());
    argv
}

/// Launch the configured external player with the right argv. Returns
/// once the spawn completes (does not wait for the player to exit).
///
/// On Unix the child inherits a closed stdin/stdout/stderr; the parent
/// never `wait()`s on it, so when ani-gui exits the child is reparented
/// to init (PID 1) and continues independently. That's the behavior
/// `ani-cli`'s `nohup ... &` invocation gives, sufficient for our needs.
/// On Windows, `Command::spawn` without `Wait()` already detaches.
///
/// # Errors
/// - [`AniError::MissingBinary`] if the configured player command can't
///   be spawned (usually means it's not on PATH).
pub fn open_external_player(args: &LaunchArgs) -> Result<()> {
    if args.player_command.trim().is_empty() {
        return Err(AniError::MissingBinary);
    }
    let argv = build_argv(args);
    let mut cmd = std::process::Command::new(&args.player_command);
    cmd.args(&argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|_| AniError::MissingBinary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(stream: &str) -> LaunchArgs {
        LaunchArgs {
            stream_url: stream.into(),
            referer: None,
            subtitle_url: None,
            title: None,
            player_command: "mpv".into(),
        }
    }

    #[test]
    fn argv_with_only_stream_is_a_single_arg() {
        let v = build_argv(&args("https://example.com/v.mp4"));
        assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
    }

    #[test]
    fn argv_includes_force_media_title_when_present() {
        let mut a = args("https://example.com/v.mp4");
        a.title = Some("Test Anime Episode 1".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "--force-media-title=Test Anime Episode 1".to_string(),
                "https://example.com/v.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn argv_emits_sub_file_and_referer_in_the_same_order_as_ani_cli() {
        let mut a = args("https://example.com/master.m3u8");
        a.title = Some("T".into());
        a.subtitle_url = Some("https://example.com/sub.vtt".into());
        a.referer = Some("https://allmanga.to".into());
        let v = build_argv(&a);
        // Order matches play_episode's construction:
        //   --force-media-title=... --sub-file=... --referrer=... <url>
        assert_eq!(
            v,
            vec![
                "--force-media-title=T".to_string(),
                "--sub-file=https://example.com/sub.vtt".to_string(),
                "--referrer=https://allmanga.to".to_string(),
                "https://example.com/master.m3u8".to_string(),
            ]
        );
    }

    #[test]
    fn open_external_player_with_blank_command_returns_missing_binary() {
        let mut a = args("https://example.com/v.mp4");
        a.player_command = String::new();
        let r = open_external_player(&a);
        assert!(matches!(r, Err(AniError::MissingBinary)));
    }

    #[test]
    fn open_external_player_with_unknown_command_returns_missing_binary() {
        let mut a = args("https://example.com/v.mp4");
        a.player_command = "__definitely_not_a_real_player__".into();
        let r = open_external_player(&a);
        assert!(matches!(r, Err(AniError::MissingBinary)));
    }
}
