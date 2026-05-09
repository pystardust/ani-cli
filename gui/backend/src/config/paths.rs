//! XDG-aware path resolution.
//!
//! On Linux: respects `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME`,
//! `$XDG_STATE_HOME`. On macOS: uses `~/Library/Application Support` and
//! friends per `directories-next`. On Windows: `%APPDATA%`.
//!
//! `ani_cli_history` is intentionally pinned to the same path the CLI
//! writes to (`$XDG_STATE_HOME/ani-cli/ani-hsts`) so the GUI and CLI
//! share a single history file.

use std::path::PathBuf;

use directories_next::ProjectDirs;

const QUALIFIER: &str = "net";
const ORG: &str = "thirdmovement";
const APP: &str = "ani-gui";

/// Project directory bundle for ani-gui.
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORG, APP)
}

/// `$XDG_CONFIG_HOME/ani-gui/config.toml` (or platform-equivalent).
#[must_use]
pub fn config_file() -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join("config.toml"))
}

/// `$XDG_CACHE_HOME/ani-gui/`.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    project_dirs().map(|d| d.cache_dir().to_path_buf())
}

/// `$XDG_CACHE_HOME/ani-gui/images/` — backing store for the `image://`
/// custom protocol.
#[must_use]
pub fn image_cache_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("images"))
}

/// `$XDG_CACHE_HOME/ani-gui/metadata.sqlite` — SQLite metadata cache.
#[must_use]
pub fn metadata_db() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("metadata.sqlite"))
}

/// `$XDG_DATA_HOME/ani-gui/logs/`.
#[must_use]
pub fn logs_dir() -> Option<PathBuf> {
    project_dirs().map(|d| d.data_dir().join("logs"))
}

/// The history file shared with the CLI:
/// `$XDG_STATE_HOME/ani-cli/ani-hsts` on Linux. On other platforms this
/// returns the equivalent state directory under `ani-cli` (not `ani-gui`),
/// so a user who alternates between CLI and GUI sees one history.
#[must_use]
pub fn ani_cli_history() -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var("ANI_CLI_HIST_DIR") {
        return Some(PathBuf::from(override_dir).join("ani-hsts"));
    }
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        return Some(PathBuf::from(state).join("ani-cli").join("ani-hsts"));
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("ani-cli")
            .join("ani-hsts"),
    )
}

/// Default destination for episode downloads:
/// `$XDG_DOWNLOAD_DIR/ani-gui/`, falling back to `$HOME/Downloads/ani-gui/`
/// when XDG isn't set (the same fallback chain the user-dirs spec
/// recommends). Returns `None` only when neither var is available, in
/// which case the renderer should ask for a path explicitly.
///
/// The user can always override per-download via the folder picker
/// before confirming — this is just the *default* the picker opens at.
#[must_use]
pub fn download_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DOWNLOAD_DIR") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("ani-gui"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Downloads").join("ani-gui"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate process-global env vars so they
    /// don't clobber each other under cargo test's default parallelism.
    /// Multiple env vars share one lock; the granularity is "any test
    /// that touches env" rather than per-var, which is fine for the
    /// handful of tests here.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn ani_cli_history_honors_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var_os("ANI_CLI_HIST_DIR");
        std::env::set_var("ANI_CLI_HIST_DIR", "/tmp/test-ani-hsts-dir");
        let p = ani_cli_history().expect("history path resolves");
        if let Some(s) = saved {
            std::env::set_var("ANI_CLI_HIST_DIR", s);
        } else {
            std::env::remove_var("ANI_CLI_HIST_DIR");
        }
        assert!(p.ends_with("ani-hsts"));
        assert!(p.starts_with("/tmp/test-ani-hsts-dir"));
    }

    #[test]
    fn download_dir_honors_xdg_download_dir() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var_os("XDG_DOWNLOAD_DIR");
        std::env::set_var("XDG_DOWNLOAD_DIR", "/tmp/test-xdg-downloads");
        let p = download_dir().expect("download path resolves");
        if let Some(s) = saved {
            std::env::set_var("XDG_DOWNLOAD_DIR", s);
        } else {
            std::env::remove_var("XDG_DOWNLOAD_DIR");
        }
        assert_eq!(p, PathBuf::from("/tmp/test-xdg-downloads/ani-gui"));
    }

    #[test]
    fn download_dir_falls_back_to_home_downloads_without_xdg() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_xdg = std::env::var_os("XDG_DOWNLOAD_DIR");
        let saved_home = std::env::var_os("HOME");
        std::env::remove_var("XDG_DOWNLOAD_DIR");
        std::env::set_var("HOME", "/tmp/test-fake-home");
        let p = download_dir().expect("download path resolves");
        if let Some(s) = saved_xdg {
            std::env::set_var("XDG_DOWNLOAD_DIR", s);
        }
        if let Some(s) = saved_home {
            std::env::set_var("HOME", s);
        }
        assert_eq!(p, PathBuf::from("/tmp/test-fake-home/Downloads/ani-gui"));
    }

    /// `config_file` / `cache_dir` / `image_cache_dir` / `metadata_db`
    /// / `logs_dir` all derive from `project_dirs()`. We don't pin
    /// the exact host path (it depends on the platform + the dev
    /// machine's HOME); we only assert the suffix the rest of the
    /// codebase relies on.
    #[test]
    fn project_dir_derived_paths_have_expected_filenames() {
        if let Some(p) = config_file() {
            assert!(p.ends_with("config.toml"), "got {p:?}");
        }
        if let Some(p) = cache_dir() {
            assert!(p.is_absolute(), "cache_dir must be absolute: {p:?}");
        }
        if let Some(p) = image_cache_dir() {
            assert!(p.ends_with("images"), "got {p:?}");
        }
        if let Some(p) = metadata_db() {
            assert!(p.ends_with("metadata.sqlite"), "got {p:?}");
        }
        if let Some(p) = logs_dir() {
            assert!(p.ends_with("logs"), "got {p:?}");
        }
    }

    /// `ani_cli_history` honours `XDG_STATE_HOME` when no override is
    /// set — it composes `<state>/ani-cli/ani-hsts`.
    #[test]
    fn ani_cli_history_honors_xdg_state_home() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_override = std::env::var_os("ANI_CLI_HIST_DIR");
        let saved_state = std::env::var_os("XDG_STATE_HOME");
        std::env::remove_var("ANI_CLI_HIST_DIR");
        std::env::set_var("XDG_STATE_HOME", "/tmp/test-state");
        let p = ani_cli_history().expect("history path resolves");
        if let Some(s) = saved_override {
            std::env::set_var("ANI_CLI_HIST_DIR", s);
        }
        if let Some(s) = saved_state {
            std::env::set_var("XDG_STATE_HOME", s);
        } else {
            std::env::remove_var("XDG_STATE_HOME");
        }
        assert_eq!(
            p,
            PathBuf::from("/tmp/test-state")
                .join("ani-cli")
                .join("ani-hsts")
        );
    }

    /// `ani_cli_history` falls back to `$HOME/.local/state/ani-cli/`
    /// when neither env var is set. Pin the layout because the CLI
    /// reads from the same path — drift would silently split history.
    #[test]
    fn ani_cli_history_falls_back_to_home_local_state() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_override = std::env::var_os("ANI_CLI_HIST_DIR");
        let saved_state = std::env::var_os("XDG_STATE_HOME");
        let saved_home = std::env::var_os("HOME");
        std::env::remove_var("ANI_CLI_HIST_DIR");
        std::env::remove_var("XDG_STATE_HOME");
        std::env::set_var("HOME", "/tmp/test-fake-home");
        let p = ani_cli_history().expect("history path resolves");
        if let Some(s) = saved_override {
            std::env::set_var("ANI_CLI_HIST_DIR", s);
        }
        if let Some(s) = saved_state {
            std::env::set_var("XDG_STATE_HOME", s);
        }
        if let Some(s) = saved_home {
            std::env::set_var("HOME", s);
        } else {
            std::env::remove_var("HOME");
        }
        assert_eq!(
            p,
            PathBuf::from("/tmp/test-fake-home/.local/state/ani-cli/ani-hsts")
        );
    }

    /// Empty XDG_DOWNLOAD_DIR is treated as unset — without this
    /// guard, a misconfigured env (some Linux distros export
    /// `XDG_DOWNLOAD_DIR=`) would resolve to `/ani-gui`.
    #[test]
    fn download_dir_treats_empty_xdg_as_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_xdg = std::env::var_os("XDG_DOWNLOAD_DIR");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("XDG_DOWNLOAD_DIR", "");
        std::env::set_var("HOME", "/tmp/test-fake-home");
        let p = download_dir().expect("download path resolves");
        if let Some(s) = saved_xdg {
            std::env::set_var("XDG_DOWNLOAD_DIR", s);
        } else {
            std::env::remove_var("XDG_DOWNLOAD_DIR");
        }
        if let Some(s) = saved_home {
            std::env::set_var("HOME", s);
        }
        assert_eq!(p, PathBuf::from("/tmp/test-fake-home/Downloads/ani-gui"));
    }

    #[test]
    fn download_dir_returns_none_when_no_home_no_xdg() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_xdg = std::env::var_os("XDG_DOWNLOAD_DIR");
        let saved_home = std::env::var_os("HOME");
        std::env::remove_var("XDG_DOWNLOAD_DIR");
        std::env::remove_var("HOME");
        let p = download_dir();
        if let Some(s) = saved_xdg {
            std::env::set_var("XDG_DOWNLOAD_DIR", s);
        }
        if let Some(s) = saved_home {
            std::env::set_var("HOME", s);
        }
        assert!(p.is_none(), "expected None without HOME/XDG, got {p:?}");
    }
}
