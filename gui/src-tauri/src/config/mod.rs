//! Settings persisted to `$XDG_CONFIG_HOME/ani-gui/config.toml`.
//!
//! User-overridable values: locale, default quality, sub/dub mode,
//! external player command, image cache cap, etc.

pub mod paths;

use serde::{Deserialize, Serialize};

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            mode: "sub".into(),
            quality: "best".into(),
            external_player: "mpv".into(),
            image_cache_cap_mb: 500,
        }
    }
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
    fn round_trips_through_toml() {
        let c = Config {
            locale: "pt-BR".into(),
            ..Config::default()
        };
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(c, parsed);
    }
}
