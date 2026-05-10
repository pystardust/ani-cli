//! Settings persisted to `$XDG_CONFIG_HOME/ani-gui/config.toml`.
//!
//! User-overridable values: locale, default quality, sub/dub mode,
//! external player command, image cache cap, etc.

pub mod paths;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

/// Application configuration. Values default to upstream `ani-cli`'s
/// defaults so a fresh install behaves identically until the user opens
/// Settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case")]
pub struct Config {
    /// UI locale (BCP 47). Default `"en"`.
    pub locale: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// Quality token: `"best"`, `"worst"`, `"1080"`, etc.
    pub quality: String,
    /// External-player command for the escape hatch. Default `"mpv"`.
    pub external_player: String,
    /// Hard cap on the on-disk image cache, in megabytes.
    pub image_cache_cap_mb: u64,
    /// When `true`, the player auto-advances to the next episode at
    /// `ended`. Opt-in — defaults to `false` so the existing behaviour
    /// (stop at end of episode) is preserved.
    pub auto_play_next: bool,
    /// When `true`, the bottom-of-screen progress strip renders while
    /// any downloads are active. Defaults to `true`. The topbar
    /// download icon + popover dock remain available either way; this
    /// setting only governs the persistent bottom-of-screen surface.
    pub download_bottom_bar_enabled: bool,
    /// When `true`, the player auto-skips opening sequences using
    /// aniskip's crowd-sourced timestamps instead of showing a
    /// manual Skip button. Opt-in — most users want to read /
    /// vibe with the OP at least the first time. Default `false`.
    pub auto_skip_op: bool,
    /// When `true`, the player auto-skips ending sequences. Same
    /// rationale as `auto_skip_op`. Default `false`.
    pub auto_skip_ed: bool,
    /// Toggles between Chromium's native `<video>` controls bar and
    /// our custom controls overlay. Custom gives the timeline the
    /// per-show accent color + lets the fullscreen button target
    /// `.player-frame` so the Skip OP/Outro overlay stays visible
    /// during fullscreen — at the cost of losing the native PiP
    /// menu and caption picker. Default `false` (native).
    pub use_custom_player_controls: bool,
    /// When `true`, navigating away from the player pauses the
    /// video instead of entering Picture-in-Picture. Auto-PiP is
    /// the default behaviour (the user keeps watching while they
    /// browse); this flag disables it for users who'd rather have
    /// navigation halt playback.
    pub disable_auto_pip_on_leave: bool,
    /// When `true`, the backend runs `ani-cli -U` against its cached
    /// copy on each boot if the script is older than 24 h. Defaults
    /// to `true` because allmanga's API drifts daily — without
    /// fresh script content, playback breaks for everyone the moment
    /// upstream pushes a hotfix we don't have. Users on metered or
    /// strictly offline setups can flip it off.
    pub auto_update_anicli: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            mode: "sub".into(),
            quality: "best".into(),
            external_player: "mpv".into(),
            image_cache_cap_mb: 500,
            auto_play_next: false,
            download_bottom_bar_enabled: true,
            auto_skip_op: false,
            auto_skip_ed: false,
            use_custom_player_controls: false,
            disable_auto_pip_on_leave: false,
            auto_update_anicli: true,
        }
    }
}

/// Read the config file at `path`. Missing-file returns
/// `Config::default()` so a fresh install behaves like an unconfigured
/// `ani-cli` user.
///
/// # Errors
/// - [`AniError::Io`] if the file exists but cannot be read.
/// - [`AniError::Config`] if the file isn't valid TOML or has
///   incompatible types.
pub fn read_config(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let body = std::fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&body)?;
    Ok(cfg)
}

/// Atomically write `cfg` to `path` (writes to `path.new` then renames).
/// Creates the parent directory if absent.
///
/// # Errors
/// - [`AniError::Io`] on filesystem failures.
/// - [`AniError::Config`] if TOML serialization fails (shouldn't in practice).
pub fn write_config(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
    }
    let body = toml::to_string_pretty(cfg).map_err(|_| AniError::Config)?;
    let tmp = path.with_extension("toml.new");
    std::fs::write(&tmp, body).map_err(|_| AniError::Io)?;
    std::fs::rename(&tmp, path).map_err(|_| AniError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_ani_cli_defaults() {
        let c = Config::default();
        assert_eq!(c.mode, "sub");
        assert_eq!(c.quality, "best");
        assert_eq!(c.external_player, "mpv");
    }

    #[test]
    fn auto_play_next_defaults_to_false() {
        // The toggle must be opt-in. Existing users (no field in their
        // config.toml) shouldn't suddenly find episodes auto-advancing
        // after an upgrade.
        assert!(!Config::default().auto_play_next);
    }

    #[test]
    fn auto_skip_defaults_to_false() {
        // Same opt-in rationale: existing users shouldn't suddenly
        // lose the OP/ED on upgrade. Many fans actively want to
        // hear the OP at least the first time.
        let c = Config::default();
        assert!(!c.auto_skip_op);
        assert!(!c.auto_skip_ed);
    }

    #[test]
    fn auto_play_next_round_trips_through_toml() {
        let c = Config {
            auto_play_next: true,
            ..Config::default()
        };
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert!(parsed.auto_play_next);
    }

    #[test]
    fn auto_play_next_absent_in_old_config_decodes_as_false() {
        // Pre-existing config.toml files don't have this field. Thanks
        // to #[serde(default)] on the struct they should still parse,
        // with the missing field defaulting to false.
        let body = "mode = \"sub\"\nquality = \"best\"\n";
        let cfg: Config = toml::from_str(body).unwrap();
        assert!(!cfg.auto_play_next);
    }

    #[test]
    fn round_trips_through_toml() {
        let c = Config {
            locale: "pt-BR".into(),
            ..Config::default()
        };
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(c, parsed);
    }

    #[test]
    fn external_player_kind_round_trips_through_toml() {
        // The kind picker in settings persists this value to disk; if it
        // doesn't survive a TOML round-trip, the user's choice resets
        // every launch.
        use crate::commands::external_player::ExternalPlayerKind;
        let c = Config {
            external_player_kind: ExternalPlayerKind::Vlc,
            ..Config::default()
        };
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(parsed.external_player_kind, ExternalPlayerKind::Vlc);
    }

    #[test]
    fn external_player_kind_absent_in_old_config_decodes_as_mpv() {
        // Pre-existing config.toml files don't have this field — they
        // must decode with the default Mpv kind so existing users don't
        // get a sudden behaviour change on upgrade.
        use crate::commands::external_player::ExternalPlayerKind;
        let body = "external_player = \"mpv\"\n";
        let cfg: Config = toml::from_str(body).unwrap();
        assert_eq!(cfg.external_player_kind, ExternalPlayerKind::Mpv);
    }

    #[test]
    fn external_player_custom_args_round_trips_through_toml() {
        // The Custom kind needs the args template to survive disk
        // round-trips — otherwise picking Custom and writing a template
        // resets every launch.
        let c = Config {
            external_player_custom_args: "--ref={referer} --title={title} {url}".into(),
            ..Config::default()
        };
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(
            parsed.external_player_custom_args,
            "--ref={referer} --title={title} {url}"
        );
    }

    #[test]
    fn external_player_custom_args_defaults_to_empty_string() {
        // A fresh install has nothing to spawn with under Custom — the
        // empty default is the trigger for build_argv_custom's bare-URL
        // fallback.
        let c = Config::default();
        assert!(c.external_player_custom_args.is_empty());
    }

    #[test]
    fn read_config_returns_defaults_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        let c = read_config(&path).expect("ok");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn write_then_read_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        let c = Config {
            mode: "dub".into(),
            quality: "1080".into(),
            external_player: "vlc".into(),
            ..Config::default()
        };
        write_config(&path, &c).expect("write");
        let back = read_config(&path).expect("read");
        assert_eq!(back, c);
    }

    #[test]
    fn write_config_is_atomic_via_temp_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_config(&path, &Config::default()).unwrap();
        // The .new sidecar should not survive the write.
        let sidecar = path.with_extension("toml.new");
        assert!(!sidecar.exists(), "atomic-rename leaves no .new behind");
    }

    #[test]
    fn read_config_rejects_non_toml_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not toml: [[[").unwrap();
        let r = read_config(&path);
        assert!(matches!(r, Err(AniError::Config)));
    }
}
