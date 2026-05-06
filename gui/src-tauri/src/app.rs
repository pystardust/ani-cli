//! `AppState` — the single state value Tauri hands to every command.
//!
//! Wires together everything the frontend can reach:
//!
//! - the streaming proxy (its session table, app secret, http client,
//!   origin, and the kernel-assigned base URL once the listener is up)
//! - the resolved path to `ani-cli` and a [`DebugOptions`] template the
//!   commands fill in per call
//! - the path of the shared ani-hsts history file
//! - a concurrency limiter for ani-cli invocations so we never hammer
//!   allanime
//!
//! Built once during `tauri::Builder::setup` and stored as managed state.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::anicli::process::{locate_ani_cli, DebugOptions};
use crate::cache::SqlitePool;
use crate::config::paths;
use crate::error::{AniError, Result};
use crate::meta::kitsu::KitsuClient;
use crate::proxy::{AppSecret, ProxyOrigin, ProxyState, SessionTable};

/// Maximum concurrent `ani-cli` subprocess invocations.
pub const SCRAPER_CONCURRENCY: usize = 2;

/// Single state container Tauri hands to every command.
#[derive(Clone)]
pub struct AppState {
    /// HMAC secret for stream tokens.
    pub secret: AppSecret,
    /// Live session table (shared with the proxy server).
    pub sessions: SessionTable,
    /// Outbound http client used by the proxy.
    pub proxy_http: reqwest::Client,
    /// Public base URL the frontend uses to reach the proxy
    /// (`http://127.0.0.1:<port>`). Set after the listener binds.
    pub proxy_origin: ProxyOrigin,
    /// Resolved path to the vendored `ani-cli` script.
    pub ani_cli_path: PathBuf,
    /// Path of the shared history file.
    pub history_path: PathBuf,
    /// Concurrency limiter for ani-cli subprocess spawns.
    pub scraper_slots: Arc<Semaphore>,
    /// On-disk image-cache directory served by the `image://` protocol.
    pub image_cache_dir: PathBuf,
    /// Connection pool for the SQLite metadata cache.
    pub cache_pool: SqlitePool,
    /// Kitsu metadata client (shares the same reqwest pool as the proxy).
    pub kitsu: KitsuClient,
}

impl AppState {
    /// Build state from the resolved proxy origin and the shared http
    /// client. `ani_cli_resource_dir` is the Tauri-bundled fallback path
    /// (the script is shipped as a resource); we look it up on PATH first.
    ///
    /// # Errors
    /// - [`AniError::MissingBinary`] if neither PATH nor the resource dir
    ///   has `ani-cli`.
    /// - [`AniError::Io`] if the history file's parent directory can't be
    ///   resolved (e.g., XDG paths fail on an exotic platform).
    pub fn build(
        proxy_http: reqwest::Client,
        proxy_origin: ProxyOrigin,
        ani_cli_resource_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let fallback = ani_cli_resource_dir.map(|d| d.join("ani-cli"));
        let ani_cli_path = locate_ani_cli(fallback.as_ref())?;
        let history_path = paths::ani_cli_history().ok_or(AniError::Io)?;
        let image_cache_dir = paths::image_cache_dir().ok_or(AniError::Io)?;
        std::fs::create_dir_all(&image_cache_dir).map_err(|_| AniError::Io)?;
        let metadata_db = paths::metadata_db().ok_or(AniError::Io)?;
        if let Some(parent) = metadata_db.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
        }
        let cache_pool = crate::cache::open_pool(&metadata_db)?;
        let kitsu = KitsuClient::new(proxy_http.clone());
        Ok(Self {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http,
            proxy_origin,
            ani_cli_path,
            history_path,
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir,
            cache_pool,
            kitsu,
        })
    }

    /// A fresh [`DebugOptions`] for an ani-cli invocation, picking up the
    /// right script path and history dir from this state.
    #[must_use]
    pub fn debug_options(&self) -> DebugOptions {
        let mut opts = DebugOptions::new(self.ani_cli_path.clone());
        // history_path is `…/ani-cli/ani-hsts`; ANI_CLI_HIST_DIR wants the
        // directory containing the file.
        opts.hist_dir = self.history_path.parent().map(std::path::Path::to_path_buf);
        opts
    }

    /// Convert into a [`ProxyState`] suitable for the axum router.
    #[must_use]
    pub fn proxy_state(&self) -> ProxyState {
        ProxyState {
            sessions: self.sessions.clone(),
            secret: self.secret.clone(),
            client: self.proxy_http.clone(),
            origin: self.proxy_origin.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_state() -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: PathBuf::from("/tmp/ani-cli"),
            history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
        }
    }

    #[test]
    fn proxy_state_view_shares_session_table_with_app_state() {
        let app = fake_state();
        let proxy = app.proxy_state();
        // Inserting via one view is visible from the other (same Arc<DashMap>).
        let id = proxy.sessions.insert(crate::proxy::StreamSession::new(
            url::Url::parse("https://example.com/m.m3u8").unwrap(),
            "https://allmanga.to",
            None,
        ));
        assert!(app.sessions.get(&id).is_some());
    }

    #[test]
    fn debug_options_picks_up_hist_dir_from_state() {
        let app = fake_state();
        let opts = app.debug_options();
        assert_eq!(opts.ani_cli_path, PathBuf::from("/tmp/ani-cli"));
        assert_eq!(
            opts.hist_dir.as_deref(),
            Some(std::path::Path::new("/tmp/ani-cli"))
        );
    }

    #[test]
    fn scraper_slots_starts_at_capacity() {
        let app = fake_state();
        assert_eq!(app.scraper_slots.available_permits(), SCRAPER_CONCURRENCY);
    }
}
