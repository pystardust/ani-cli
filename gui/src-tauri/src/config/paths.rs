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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ani_cli_history_honors_override() {
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
}
