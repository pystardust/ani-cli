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
