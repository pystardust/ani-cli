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
use crate::anicli::update::{self, UpdateOutcome};
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
    /// Resolved path to `bash.exe` on Windows; `None` on Unix where
    /// the script runs directly via shebang. Resolved once at startup
    /// via [`crate::anicli::bash::locate_bash`] and threaded into
    /// every spawn site via [`DebugOptions::bash_path`].
    pub bash_path: Option<PathBuf>,
    /// Directory shipped next to the backend binary holding bundled
    /// POSIX deps (Windows: `fzf.exe`). Computed once in `build()`
    /// from the resource dir; threaded into every ani-cli spawn so
    /// the script's `dep_ch` finds the bundled binaries before any
    /// system install. `None` on Unix and on Windows dev runs where
    /// the directory hasn't been laid down.
    pub bundled_bin: Option<PathBuf>,
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
    /// Path to the user's TOML settings file (`config.toml`).
    pub config_path: PathBuf,
    /// `$XDG_STATE_HOME/ani-gui/` — backing store for the latest
    /// `ani-cli -U` outcome JSON the diagnostics page reads.
    pub state_dir: PathBuf,
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
        // Bundled-deps dir lives at `<resource_dir>/bin`. On Windows
        // the NSIS installer drops `fzf.exe` here via electron-builder
        // `extraResources`; in cargo dev runs the same path is
        // populated by the `fetch:win-deps` script so playback works
        // without polluting global PATH. Computed before consuming
        // `ani_cli_resource_dir` for the script fallback below.
        let bundled_bin = ani_cli_resource_dir
            .as_ref()
            .map(|d| d.join("bin"))
            .filter(|p| p.is_dir());
        // Resolve the script path through the writable cache copy so
        // `-U` can patch it in place. The seed is whatever PATH or
        // the resource dir gives us; the live path is always under
        // `$XDG_CACHE_HOME/ani-gui/ani-cli`. The reapply pass is
        // idempotent and ensures our `__ANI_CLI_LIB__` source guard
        // survives between releases.
        let fallback = ani_cli_resource_dir.map(|d| d.join("ani-cli"));
        let seed = locate_ani_cli(fallback.as_ref())?;
        let cache_root = paths::cache_dir().ok_or(AniError::Io)?;
        let ani_cli_path = update::resolve_anicli_path(&seed, &cache_root).map_err(|e| {
            tracing::warn!(target: "anicli::boot", error = %e, "resolve_anicli_path failed; falling back to seed");
            AniError::Io
        })?;
        // Strip the bats test-loader guard from the cache copy so it
        // matches upstream's content shape; otherwise every -U would
        // report Updated as the patch keeps removing our line.
        if let Err(e) = update::strip_lib_guard(&ani_cli_path) {
            tracing::warn!(target: "anicli::boot", error = %e, "strip_lib_guard failed");
        }
        // Windows: locate Git Bash so every ani-cli spawn (regular,
        // search, `-U`) can wrap the POSIX script with bash. Surface
        // BashMissing so the frontend can render an install-Git-for-
        // Windows pointer instead of a generic missing-binary error.
        // Unix: the field stays None — the script runs via shebang.
        let bash_path = resolve_bash_path()?;
        let history_path = paths::ani_cli_history().ok_or(AniError::Io)?;
        let image_cache_dir = paths::image_cache_dir().ok_or(AniError::Io)?;
        std::fs::create_dir_all(&image_cache_dir).map_err(|_| AniError::Io)?;
        let metadata_db = paths::metadata_db().ok_or(AniError::Io)?;
        if let Some(parent) = metadata_db.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
        }
        let cache_pool = crate::cache::open_pool(&metadata_db)?;
        let kitsu = KitsuClient::new(proxy_http.clone());
        let config_path = paths::config_file().ok_or(AniError::Io)?;
        let state_dir = paths::state_dir().ok_or(AniError::Io)?;
        Ok(Self {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http,
            proxy_origin,
            ani_cli_path,
            bash_path,
            bundled_bin,
            history_path,
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir,
            cache_pool,
            kitsu,
            config_path,
            state_dir,
        })
    }

    /// Spawn a background task that runs `ani-cli -U` and appends
    /// the outcome to the rolling log for /diagnostics to read.
    /// Gated only by the `auto_update_anicli` settings toggle. The
    /// task fires off the listener-bind path so app launch is
    /// unaffected by the curl latency.
    pub fn maybe_spawn_anicli_update(self: &Arc<Self>) {
        let cfg = crate::config::read_config(&self.config_path).unwrap_or_default();
        if !cfg.auto_update_anicli {
            return;
        }
        let script = self.ani_cli_path.clone();
        let bash = self.bash_path.clone();
        let bundled_bin = self.bundled_bin.clone();
        let state_dir = self.state_dir.clone();
        tokio::spawn(async move {
            tracing::info!(target: "anicli::update", script = %script.display(), "running -U in background");
            let outcome =
                update::run_update(&script, bash.as_deref(), bundled_bin.as_deref()).await;
            tracing::info!(
                target: "anicli::update",
                status = ?outcome.status,
                duration_ms = outcome.duration_ms,
                "ani-cli -U finished"
            );
            if let Err(e) = update::append_outcome(&state_dir, &outcome) {
                tracing::warn!(target: "anicli::update", error = %e, "append_outcome failed");
            }
        });
    }

    /// Read the persisted log of recent `-U` outcomes, latest first.
    /// Empty vector when no run has happened yet.
    pub fn anicli_update_log(&self) -> std::io::Result<Vec<UpdateOutcome>> {
        update::read_outcomes(&self.state_dir)
    }

    /// A fresh [`DebugOptions`] for an ani-cli invocation, picking up the
    /// right script path, bash path, and history dir from this state.
    #[must_use]
    pub fn debug_options(&self) -> DebugOptions {
        let mut opts = DebugOptions::new(self.ani_cli_path.clone());
        opts.bash_path = self.bash_path.clone();
        opts.bundled_bin = self.bundled_bin.clone();
        // history_path is `…/ani-cli/ani-hsts`; ANI_CLI_HIST_DIR wants the
        // directory containing the file.
        opts.hist_dir = self.history_path.parent().map(std::path::Path::to_path_buf);
        opts
    }

    /// Configured image-cache size cap, in bytes. Reads from the
    /// user's settings TOML on each call (cheap; sub-millisecond)
    /// so a settings change applies immediately without restarting.
    /// Falls back to the documented default if the file is missing
    /// or unreadable.
    #[must_use]
    pub fn image_cache_cap_bytes(&self) -> u64 {
        let cfg = crate::config::read_config(&self.config_path).unwrap_or_default();
        cfg.image_cache_cap_mb.saturating_mul(1024 * 1024)
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

/// Resolve `bash.exe` at startup on Windows; return `None` on Unix
/// where the script runs via shebang. On Windows, `Err(BashMissing)`
/// when no Git for Windows install is reachable so the frontend can
/// render an install pointer.
fn resolve_bash_path() -> Result<Option<PathBuf>> {
    #[cfg(windows)]
    {
        match crate::anicli::bash::locate_bash() {
            Some(p) => Ok(Some(p)),
            None => Err(AniError::BashMissing),
        }
    }
    #[cfg(not(windows))]
    {
        Ok(None)
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
            bash_path: None,
            bundled_bin: None,
            history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
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
    fn debug_options_threads_bash_path_from_state() {
        // Windows-readiness: the AppState's resolved bash path must
        // flow into every spawn site via DebugOptions. Linux fakes
        // None; setting a path on the state proves the threading is
        // wired through.
        let mut app = fake_state();
        app.bash_path = Some(PathBuf::from("/opt/git/bin/bash.exe"));
        let opts = app.debug_options();
        assert_eq!(
            opts.bash_path.as_deref(),
            Some(std::path::Path::new("/opt/git/bin/bash.exe"))
        );
    }

    #[test]
    fn debug_options_carries_none_bash_path_on_unix_default() {
        // The default fake_state has no bash configured; debug_options
        // must propagate that None so the spawn helper runs the
        // script directly via shebang on Unix.
        let app = fake_state();
        assert!(app.debug_options().bash_path.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn resolve_bash_path_returns_ok_none_on_unix() {
        // On Unix, no bash lookup happens — the script runs via
        // shebang. The helper must return Ok(None) to keep the
        // optional-field invariant.
        let got = resolve_bash_path().expect("resolve_bash_path should not error on Unix");
        assert!(got.is_none(), "Unix expects None, got {got:?}");
    }

    #[test]
    fn scraper_slots_starts_at_capacity() {
        let app = fake_state();
        assert_eq!(app.scraper_slots.available_permits(), SCRAPER_CONCURRENCY);
    }
}
