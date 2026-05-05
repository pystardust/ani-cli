//! Spawn the `ani-cli` script as a subprocess.
//!
//! All invocations:
//!
//! - clear the inherited environment except `PATH`, `HOME`, `XDG_*`,
//!   `ANI_CLI_HIST_DIR`, and a few other test-relevant overrides
//! - set `TERM=dumb`, `NO_COLOR=1` to suppress color and cursor escapes
//! - set `kill_on_drop(true)` so cancelled futures don't leak shell PIDs
//! - bound by a wall-clock timeout
//! - read stdout fully, strip ANSI, parse via [`super::parser`]
//!
//! The function signatures here are stubs — the bodies are filled in as
//! M1.2 progresses with TDD coverage.

use std::path::PathBuf;
use std::time::Duration;

use crate::anicli::parser::{DebugOutput, SearchResult};
use crate::error::{AniError, Result};

/// How long any single `ani-cli` invocation may run before it is killed.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Locate the `ani-cli` binary. Looks at `$PATH`, then falls back to a
/// path passed by the caller (typically the Tauri resource directory).
///
/// # Errors
/// Returns [`AniError::MissingBinary`] when no executable is found.
pub fn locate_ani_cli(fallback: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(found) = find_in_path("ani-cli") {
        return Ok(found);
    }
    if let Some(p) = fallback {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    Err(AniError::MissingBinary)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(p: &std::path::Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("cmd"))
            .unwrap_or(false)
}

/// Run `ani-cli` in debug-player mode and return the parsed output. Stub —
/// implemented in M1.2 with integration tests behind a curl shim.
///
/// # Errors
/// Will return [`AniError`] variants for spawn failure, timeout, non-zero
/// exit, and parse failure.
pub async fn run_debug(
    _query: &str,
    _ep: &str,
    _quality: &str,
    _mode: &str,
) -> Result<DebugOutput> {
    // Intentional async-to-sync stub: real implementation calls
    // tokio::process::Command which is async.
    let _ = tokio::task::yield_now().await;
    Err(AniError::MissingBinary)
}

/// Run `ani-cli` in search mode (early-exit before episode prompt) and
/// return the parsed result list. Stub — implemented in M1.2.
///
/// # Errors
/// See [`run_debug`].
pub async fn run_search(_query: &str, _mode: &str) -> Result<Vec<SearchResult>> {
    let _ = tokio::task::yield_now().await;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_ani_cli_with_no_path_and_no_fallback_errors() {
        // Save and clear $PATH so `which` cannot find ani-cli.
        let saved = std::env::var_os("PATH");
        // Use unsafe-free API: the std::env::set_var on stable is safe. The
        // test mutates process global state, but the test is single-threaded
        // (cargo test default) so no race.
        std::env::set_var("PATH", "");
        let r = locate_ani_cli(None);
        if let Some(p) = saved {
            std::env::set_var("PATH", p);
        }
        assert!(matches!(r, Err(AniError::MissingBinary)));
    }
}
