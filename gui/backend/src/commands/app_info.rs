//! `app_info` command — meta about the running backend.

use serde::Serialize;

use crate::error::Result;

/// Snapshot of build/runtime info the frontend may want at startup.
#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    /// Crate version from Cargo.toml.
    pub version: &'static str,
    /// Detected `ani-cli` script path.
    pub ani_cli_path: String,
    /// Where this build of ani-gui keeps its history (shared with the CLI).
    pub history_path: String,
    /// `http://127.0.0.1:<port>` for the streaming proxy.
    pub proxy_base_url: String,
}

/// Body of the command. Pure projection of `AppState` fields.
///
/// # Errors
/// Currently never returns an error; signature uses `Result` to keep the
/// future-compatible shape Tauri commands expect.
pub fn app_info(state: &crate::app::AppState) -> Result<AppInfo> {
    Ok(AppInfo {
        version: crate::VERSION,
        ani_cli_path: state.ani_cli_path.display().to_string(),
        history_path: state.history_path.display().to_string(),
        proxy_base_url: state.proxy_origin.base.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    fn fake_state() -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 42_337),
            ani_cli_path: PathBuf::from("/usr/local/bin/ani-cli"),
            bash_path: None,
            bundled_bin: None,
            history_path: PathBuf::from("/home/u/.local/state/ani-cli/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(2)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
        }
    }

    #[test]
    fn app_info_projects_state_fields() {
        let s = fake_state();
        let info = app_info(&s).unwrap();
        assert_eq!(info.version, crate::VERSION);
        assert_eq!(info.ani_cli_path, "/usr/local/bin/ani-cli");
        assert_eq!(info.history_path, "/home/u/.local/state/ani-cli/ani-hsts");
        assert_eq!(info.proxy_base_url, "http://127.0.0.1:42337");
    }

    #[test]
    fn app_info_serializes_with_snake_case_unchanged() {
        let s = fake_state();
        let info = app_info(&s).unwrap();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"ani_cli_path\""));
        assert!(json.contains("\"history_path\""));
        assert!(json.contains("\"proxy_base_url\""));
    }
}
