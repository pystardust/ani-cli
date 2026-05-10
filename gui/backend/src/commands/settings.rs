//! Settings IPC commands — TOML round-trip backed by `config::Config`.
//!
//! Reads return the on-disk file, falling back to defaults when the
//! file is absent. Writes are atomic (`.new` + rename) so a crash mid-
//! save can't leave a half-written config.

use crate::app::AppState;
use crate::config::{read_config, write_config, Config};
use crate::error::Result;

/// Read the current settings. Returns [`Config::default`] when the
/// settings file doesn't exist yet (fresh install).
///
/// # Errors
/// Inherits from [`crate::config::read_config`].
pub fn settings_get(state: &AppState) -> Result<Config> {
    read_config(&state.config_path)
}

/// Replace the settings file with `cfg`. Atomic.
///
/// # Errors
/// Inherits from [`crate::config::write_config`].
pub fn settings_put(state: &AppState, cfg: &Config) -> Result<()> {
    write_config(&state.config_path, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, SCRAPER_CONCURRENCY};
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Semaphore;

    fn state_with_config_at(config_path: PathBuf) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: PathBuf::from("/x"),
            bash_path: None,
            bundled_bin: None,
            history_path: PathBuf::from("/y/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path,
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
        }
    }

    #[test]
    fn settings_get_returns_defaults_on_fresh_install() {
        let dir = TempDir::new().unwrap();
        let state = state_with_config_at(dir.path().join("config.toml"));
        let cfg = settings_get(&state).expect("ok");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn settings_put_then_get_round_trips() {
        let dir = TempDir::new().unwrap();
        let state = state_with_config_at(dir.path().join("config.toml"));
        let new_cfg = Config {
            mode: "dub".into(),
            quality: "720".into(),
            external_player: "vlc".into(),
            ..Config::default()
        };
        settings_put(&state, &new_cfg).expect("write");
        let back = settings_get(&state).expect("read");
        assert_eq!(back, new_cfg);
    }

    #[test]
    fn settings_put_creates_parent_directory_if_missing() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("never-existed").join("config.toml");
        assert!(!nested.parent().unwrap().exists());
        let state = state_with_config_at(nested.clone());
        settings_put(&state, &Config::default()).expect("creates parent");
        assert!(nested.exists());
    }
}
