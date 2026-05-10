//! `open_external_player` command — escape hatch that launches the
//! user's chosen external media player (default `mpv`) with the same
//! `--referer` and `--sub-file` flags `ani-cli` passes today.
//!
//! This is never an automatic fallback — it's user-triggered (a button
//! on the in-window player chrome). Auto-fallback would be confusing.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

/// Player flavor — controls which flag syntax `build_argv` emits.
///
/// The argv contract differs per player: mpv accepts
/// `--force-media-title=`, VLC accepts `--meta-title=`, IINA forwards
/// mpv flags via `--mpv-` prefixes, and Custom plays it safe by
/// passing only the URL (we don't know what the user's player wants).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPlayerKind {
    /// mpv — the default, matches the upstream `ani-cli` flag set.
    #[default]
    Mpv,
    /// VideoLAN VLC — different flag names for the same concepts.
    Vlc,
    /// IINA on macOS — wraps mpv, takes flags via `--mpv-` prefix.
    Iina,
    /// Anything else — bare URL only, no flags.
    Custom,
}

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
    /// Which player flag syntax to use. Old payloads without this
    /// field decode as `Mpv` so existing clients keep working.
    #[serde(default)]
    pub player_kind: ExternalPlayerKind,
    /// Free-text args template used only when `player_kind` is
    /// `Custom`. Tokens supported: `{url}`, `{referer}`, `{title}`,
    /// `{sub}`. A token containing a missing/empty placeholder is
    /// dropped from argv entirely (so optional flags don't end up
    /// as `--sub-file=` with nothing after the equals).
    #[serde(default)]
    pub custom_args_template: Option<String>,
}

/// Build the argv that would be passed to `Command::new(player).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// Order across all kinds: title, sub, referrer, URL last. Matches
/// what `ani-cli`'s `play_episode` mpv branch constructs (lines
/// 394-402 of the script).
#[must_use]
pub fn build_argv(args: &LaunchArgs) -> Vec<String> {
    match args.player_kind {
        ExternalPlayerKind::Mpv => {
            build_argv_with_template(args, "--force-media-title=", "--sub-file=", "--referrer=")
        }
        ExternalPlayerKind::Vlc => {
            build_argv_with_template(args, "--meta-title=", "--sub-file=", "--http-referrer=")
        }
        ExternalPlayerKind::Iina => build_argv_with_template(
            args,
            "--mpv-force-media-title=",
            "--sub-file=",
            "--mpv-referrer=",
        ),
        ExternalPlayerKind::Custom => build_argv_custom(args),
    }
}

/// Shared argv assembly for the three known players — same shape,
/// different flag names.
fn build_argv_with_template(
    args: &LaunchArgs,
    title_flag: &str,
    sub_flag: &str,
    referrer_flag: &str,
) -> Vec<String> {
    let mut argv = Vec::with_capacity(4);
    if let Some(t) = &args.title {
        argv.push(format!("{title_flag}{t}"));
    }
    if let Some(s) = &args.subtitle_url {
        argv.push(format!("{sub_flag}{s}"));
    }
    if let Some(r) = &args.referer {
        argv.push(format!("{referrer_flag}{r}"));
    }
    argv.push(args.stream_url.clone());
    argv
}

/// Build argv for the Custom kind by shlex-splitting the template
/// and substituting placeholders per token. A token containing a
/// missing/empty placeholder is dropped from argv entirely so the
/// user can write `--sub={sub}` without it landing as `--sub=` when
/// no subtitle is available.
///
/// Empty/None template falls back to URL only.
fn build_argv_custom(args: &LaunchArgs) -> Vec<String> {
    let template = match args.custom_args_template.as_deref() {
        Some(s) if !s.trim().is_empty() => s,
        _ => return vec![args.stream_url.clone()],
    };
    let tokens = match shlex::split(template) {
        Some(t) => t,
        // Bad quoting in the template — fall back to bare URL so the
        // user at least sees the stream open instead of silently
        // failing.
        None => return vec![args.stream_url.clone()],
    };
    let referer = args.referer.as_deref().unwrap_or("");
    let title = args.title.as_deref().unwrap_or("");
    let sub = args.subtitle_url.as_deref().unwrap_or("");
    let url = args.stream_url.as_str();
    tokens
        .into_iter()
        .filter_map(|tok| substitute_token(&tok, url, referer, title, sub))
        .collect()
}

/// Returns `Some(rendered)` when every placeholder in `tok` had a
/// non-empty value, `None` if any placeholder was empty (drop rule).
/// `{url}` is always present — tokens containing only `{url}` always
/// render. Unknown `{...}` placeholders pass through verbatim.
fn substitute_token(tok: &str, url: &str, referer: &str, title: &str, sub: &str) -> Option<String> {
    let mut out = String::with_capacity(tok.len());
    let mut chars = tok.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Read placeholder name up to `}`.
        let mut name = String::new();
        let mut closed = false;
        for nc in chars.by_ref() {
            if nc == '}' {
                closed = true;
                break;
            }
            name.push(nc);
        }
        if !closed {
            // Unterminated `{...` — pass through literally.
            out.push('{');
            out.push_str(&name);
            continue;
        }
        let value = match name.as_str() {
            "url" => url,
            "referer" => referer,
            "title" => title,
            "sub" => sub,
            // Unknown placeholder — preserve verbatim.
            other => {
                out.push('{');
                out.push_str(other);
                out.push('}');
                continue;
            }
        };
        if value.is_empty() {
            // Drop the entire token: a flag with an empty value is
            // worse than no flag at all.
            return None;
        }
        out.push_str(value);
    }
    Some(out)
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
            player_kind: ExternalPlayerKind::Mpv,
            custom_args_template: None,
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
    fn argv_for_vlc_uses_vlc_flag_syntax() {
        // VLC's flag names differ from mpv: `--meta-title` for the
        // title, `--http-referrer` for the Referer header, and the
        // global `--sub-file` for subtitles. Order matches mpv's:
        // title, sub, referrer, URL last.
        let mut a = args("https://example.com/master.m3u8");
        a.player_kind = ExternalPlayerKind::Vlc;
        a.title = Some("T".into());
        a.subtitle_url = Some("https://example.com/sub.vtt".into());
        a.referer = Some("https://allmanga.to".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "--meta-title=T".to_string(),
                "--sub-file=https://example.com/sub.vtt".to_string(),
                "--http-referrer=https://allmanga.to".to_string(),
                "https://example.com/master.m3u8".to_string(),
            ]
        );
    }

    #[test]
    fn argv_for_iina_uses_mpv_prefixed_flags() {
        // IINA wraps mpv on macOS and forwards flags through `--mpv-`,
        // except `--sub-file` which IINA exposes natively.
        let mut a = args("https://example.com/v.mp4");
        a.player_kind = ExternalPlayerKind::Iina;
        a.title = Some("T".into());
        a.subtitle_url = Some("https://example.com/sub.vtt".into());
        a.referer = Some("https://allmanga.to".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "--mpv-force-media-title=T".to_string(),
                "--sub-file=https://example.com/sub.vtt".to_string(),
                "--mpv-referrer=https://allmanga.to".to_string(),
                "https://example.com/v.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn argv_for_custom_kind_substitutes_placeholders() {
        // Custom uses a free-text template the user controls. Tokens
        // are shlex-split, then `{url}`, `{referer}`, `{title}`,
        // `{sub}` are interpolated per token.
        let mut a = args("https://example.com/v.mp4");
        a.player_kind = ExternalPlayerKind::Custom;
        a.title = Some("My Show".into());
        a.subtitle_url = Some("https://example.com/sub.vtt".into());
        a.referer = Some("https://allmanga.to".into());
        a.custom_args_template = Some("--ref={referer} --title={title} --sub={sub} {url}".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "--ref=https://allmanga.to".to_string(),
                "--title=My Show".to_string(),
                "--sub=https://example.com/sub.vtt".to_string(),
                "https://example.com/v.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn argv_for_custom_drops_tokens_with_missing_placeholders() {
        // If the user includes `--sub={sub}` in the template but the
        // current episode has no subtitle, the entire token is
        // dropped — better than emitting `--sub=` with empty value.
        let mut a = args("https://example.com/v.mp4");
        a.player_kind = ExternalPlayerKind::Custom;
        a.referer = Some("https://allmanga.to".into());
        // No subtitle, no title.
        a.custom_args_template = Some("--ref={referer} --title={title} --sub={sub} {url}".into());
        let v = build_argv(&a);
        // --title= and --sub= tokens are dropped because their
        // placeholders are missing.
        assert_eq!(
            v,
            vec![
                "--ref=https://allmanga.to".to_string(),
                "https://example.com/v.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn argv_for_custom_with_empty_template_falls_back_to_url_only() {
        // A user who picks Custom but leaves the template blank gets
        // a bare URL — not a panic, not an error.
        let mut a = args("https://example.com/v.mp4");
        a.player_kind = ExternalPlayerKind::Custom;
        a.custom_args_template = None;
        let v = build_argv(&a);
        assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
    }

    #[test]
    fn launch_args_decode_without_player_kind_field_for_back_compat() {
        // Old client payloads (pre-multi-player) don't include
        // `player_kind`. They must still decode and default to Mpv.
        let json = r#"{
            "stream_url": "https://example.com/v.mp4",
            "referer": null,
            "subtitle_url": null,
            "title": null,
            "player_command": "mpv"
        }"#;
        let a: LaunchArgs = serde_json::from_str(json).expect("decodes with default kind");
        assert_eq!(a.player_kind, ExternalPlayerKind::Mpv);
        assert!(a.custom_args_template.is_none());
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
