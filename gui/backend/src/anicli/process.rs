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

/// How `run_debug` finds the `ani-cli` script. Resolved once at startup and
/// reused per invocation.
#[derive(Debug, Clone)]
pub struct DebugOptions {
    /// Absolute path to the `ani-cli` script. Use [`locate_ani_cli`].
    pub ani_cli_path: PathBuf,
    /// Optional override for the history directory (`ANI_CLI_HIST_DIR`).
    /// Defaults to the user's `$XDG_STATE_HOME/ani-cli/` per ani-cli.
    pub hist_dir: Option<PathBuf>,
    /// Wall-clock timeout. Defaults to [`DEFAULT_TIMEOUT`].
    pub timeout: Duration,
    /// Override `PATH` (mainly for tests that put a curl shim ahead of
    /// system binaries). Defaults to the inherited `PATH`.
    pub path_override: Option<String>,
}

impl DebugOptions {
    /// Construct from a located ani-cli path with all defaults.
    #[must_use]
    pub fn new(ani_cli_path: PathBuf) -> Self {
        Self {
            ani_cli_path,
            hist_dir: None,
            timeout: DEFAULT_TIMEOUT,
            path_override: None,
        }
    }
}

/// Run `ani-cli` in debug-player mode and return the parsed output.
///
/// The script is invoked with `ANI_CLI_PLAYER=debug` so it prints the
/// candidate links and selected URL to stdout instead of launching a
/// player. The environment is scrubbed (only safe vars propagate),
/// `TERM=dumb` and `NO_COLOR=1` suppress ANSI noise, and `kill_on_drop`
/// is enabled so cancelled futures don't leak shell PIDs.
///
/// # Errors
/// - [`AniError::Timeout`] if the wall-clock timeout elapses
/// - [`AniError::Scraper`] for non-zero exit with a known stderr pattern
/// - [`AniError::ParseFailed`] if the debug stdout doesn't contain
///   `Selected link:` (the marker the script's debug branch emits)
/// - [`AniError::MissingBinary`] if `ani-cli` cannot be spawned
pub async fn run_debug(
    opts: &DebugOptions,
    query: &str,
    ep: &str,
    quality: &str,
    mode: &str,
    select_index: usize,
) -> Result<DebugOutput> {
    use tokio::process::Command;

    // ani-cli's `-S` flag is 1-based; the caller passes 1 to keep the
    // legacy "first match" behaviour or a higher index after running
    // its own search disambiguation (see `crate::scraper::allanime`).
    let select_str = select_index.max(1).to_string();

    let mut cmd = Command::new(&opts.ani_cli_path);
    cmd.arg("-S")
        .arg(&select_str)
        .arg("-e")
        .arg(ep)
        .arg("-q")
        .arg(quality);
    if mode == "dub" {
        cmd.arg("--dub");
    }
    cmd.arg("--").arg(query);

    cmd.env_clear();
    // PATH is required so ani-cli can find curl/openssl/fzf/mpv. Tests
    // override this to inject a curl shim ahead of system binaries.
    let path_value = opts
        .path_override
        .clone()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_else(|| "/usr/bin:/bin".to_string());
    cmd.env("PATH", path_value);
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
    cmd.env("TERM", "dumb");
    cmd.env("NO_COLOR", "1");
    cmd.env("ANI_CLI_PLAYER", "debug");
    if let Some(dir) = &opts.hist_dir {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    } else if let Some(dir) = std::env::var_os("ANI_CLI_HIST_DIR") {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|_| AniError::MissingBinary)?;

    let stdout_reader = child.stdout.take().expect("stdout piped");
    let stderr_reader = child.stderr.take().expect("stderr piped");

    let collected = tokio::time::timeout(opts.timeout, async move {
        let stdout_fut = read_to_end(stdout_reader);
        let stderr_fut = read_to_end(stderr_reader);
        let (out, err) = tokio::join!(stdout_fut, stderr_fut);
        let status = child.wait().await?;
        Result::<(Vec<u8>, Vec<u8>, std::process::ExitStatus)>::Ok((out?, err?, status))
    })
    .await
    .map_err(|_| AniError::Timeout)??;

    let (stdout_bytes, stderr_bytes, exit) = collected;

    if !exit.success() {
        let stderr_text = super::parser::strip_ansi(&stderr_bytes);
        let stdout_text = super::parser::strip_ansi(&stdout_bytes);
        tracing::error!(
            exit = ?exit.code(),
            stderr = %stderr_text,
            stdout = %stdout_text,
            "anicli: non-zero exit",
        );
        if stderr_text.contains("No results found") {
            return Err(AniError::NoResults);
        }
        if stderr_text.contains("Episode not released") {
            return Err(AniError::Scraper {
                key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
            });
        }
        return Err(AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
        });
    }

    let stdout_text = super::parser::strip_ansi(&stdout_bytes);
    super::parser::parse_debug_output(&stdout_text)
}

async fn read_to_end<R: tokio::io::AsyncRead + Unpin>(mut r: R) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(4096);
    r.read_to_end(&mut buf).await?;
    Ok(buf)
}

/// Variant of [`run_debug`] that calls `on_stderr_line` for every line
/// the script emits on stderr while it runs. Used by the SSE play
/// endpoint to forward `<provider> Links Fetched` progress to the
/// renderer in real time.
///
/// The callback receives lines **with ANSI escapes stripped**, in the
/// order they arrive. It MUST NOT block — the line reader awaits its
/// completion before pulling the next chunk from the pipe, so a slow
/// callback stalls the subprocess.
///
/// On exit, the subprocess's stdout is parsed exactly as in
/// [`run_debug`] and returned. Errors are mapped the same way.
///
/// # Errors
/// Same as [`run_debug`].
pub async fn run_debug_streaming<F>(
    opts: &DebugOptions,
    query: &str,
    ep: &str,
    quality: &str,
    mode: &str,
    select_index: usize,
    mut on_stderr_line: F,
) -> Result<super::parser::DebugOutput>
where
    F: FnMut(&str) + Send,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let select_str = select_index.max(1).to_string();

    let mut cmd = Command::new(&opts.ani_cli_path);
    cmd.arg("-S")
        .arg(&select_str)
        .arg("-e")
        .arg(ep)
        .arg("-q")
        .arg(quality);
    if mode == "dub" {
        cmd.arg("--dub");
    }
    cmd.arg("--").arg(query);

    cmd.env_clear();
    let path_value = opts
        .path_override
        .clone()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_else(|| "/usr/bin:/bin".to_string());
    cmd.env("PATH", path_value);
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
    cmd.env("TERM", "dumb");
    cmd.env("NO_COLOR", "1");
    cmd.env("ANI_CLI_PLAYER", "debug");
    if let Some(dir) = &opts.hist_dir {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    } else if let Some(dir) = std::env::var_os("ANI_CLI_HIST_DIR") {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|_| AniError::MissingBinary)?;

    let stdout_reader = child.stdout.take().expect("stdout piped");
    let stderr_reader = child.stderr.take().expect("stderr piped");

    // Read stderr line-by-line and forward each (ANSI-stripped) line
    // to the caller. Buffer stderr bytes too so the existing
    // post-exit error handling (No results found / Episode not
    // released) keeps working.
    let stderr_collected: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected_for_reader = stderr_collected.clone();

    let stream_fut = async {
        let mut reader = BufReader::new(stderr_reader);
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = reader.read_line(&mut buf).await?;
            if read == 0 {
                break;
            }
            // Persist the raw bytes for the post-exit error check.
            {
                let mut lock = collected_for_reader.lock().expect("mutex");
                lock.extend_from_slice(buf.as_bytes());
            }
            let stripped = super::parser::strip_ansi(buf.as_bytes());
            for line in stripped.lines() {
                on_stderr_line(line);
            }
        }
        std::io::Result::Ok(())
    };

    let collected = tokio::time::timeout(opts.timeout, async move {
        let stdout_fut = read_to_end(stdout_reader);
        let (out, err_io) = tokio::join!(stdout_fut, stream_fut);
        err_io?;
        let status = child.wait().await?;
        Result::<(Vec<u8>, std::process::ExitStatus)>::Ok((out?, status))
    })
    .await
    .map_err(|_| AniError::Timeout)??;

    let (stdout_bytes, exit) = collected;
    let stderr_bytes = stderr_collected.lock().expect("mutex").clone();

    if !exit.success() {
        let stderr_text = super::parser::strip_ansi(&stderr_bytes);
        let stdout_text = super::parser::strip_ansi(&stdout_bytes);
        tracing::error!(
            exit = ?exit.code(),
            stderr = %stderr_text,
            stdout = %stdout_text,
            "anicli (streaming): non-zero exit",
        );
        if stderr_text.contains("No results found") {
            return Err(AniError::NoResults);
        }
        if stderr_text.contains("Episode not released") {
            return Err(AniError::Scraper {
                key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
            });
        }
        return Err(AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
        });
    }

    let stdout_text = super::parser::strip_ansi(&stdout_bytes);
    super::parser::parse_debug_output(&stdout_text)
}

/// Run `ani-cli` in search mode and return the parsed result list. Stub
/// pending either an upstream `--list-only` flag or migrating GUI search
/// to Kitsu metadata (the planned M2 path). See
/// `.planning/cli-contract-deviations.md` for the full rationale.
///
/// # Errors
/// Always returns `Ok(Vec::new())` until the deviation is resolved.
pub async fn run_search(_query: &str, _mode: &str) -> Result<Vec<SearchResult>> {
    let _ = tokio::task::yield_now().await;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate process-global env (PATH) with
    /// tests that fork subprocesses (whose runtime resolves PATH at
    /// spawn time on some kernels). Without this lock the suite flaked
    /// at ~40% under `cargo test`'s default parallelism. Tokio mutex
    /// because the guard crosses `.await` points.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn locate_ani_cli_with_no_path_and_no_fallback_errors() {
        let _guard = ENV_LOCK.lock().await;
        // Save and clear $PATH so `which` cannot find ani-cli.
        let saved = std::env::var_os("PATH");
        // Use unsafe-free API: the std::env::set_var on stable is safe. The
        // test mutates process global state; the lock above keeps
        // subprocess-spawning tests out while PATH is empty.
        std::env::set_var("PATH", "");
        let r = locate_ani_cli(None);
        if let Some(p) = saved {
            std::env::set_var("PATH", p);
        }
        assert!(matches!(r, Err(AniError::MissingBinary)));
    }

    /// Build a stub `ani-cli` script that emits `stderr_msg` and exits
    /// with `code`. Returned tempdir keeps the file alive for the test.
    fn stub_ani_cli(stderr_msg: &str, code: i32) -> (tempfile::TempDir, PathBuf) {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let td = tempfile::tempdir().expect("tempdir");
        let path = td.path().join("ani-cli");
        let mut f = std::fs::File::create(&path).expect("create stub");
        // POSIX sh: forward stderr_msg to stderr, exit with the requested
        // code. Quoting `stderr_msg` is safe because we only ever pass
        // hard-coded fixture strings here.
        writeln!(f, "#!/bin/sh\necho \"{stderr_msg}\" 1>&2\nexit {code}").expect("write stub");
        let mut perm = f.metadata().expect("perm").permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&path, perm).expect("chmod");
        (td, path)
    }

    fn debug_opts(path: PathBuf) -> DebugOptions {
        let mut opts = DebugOptions::new(path);
        // Pin PATH so the parallel `locate_ani_cli_*` test (which
        // temporarily empties $PATH) can't race-clear our subprocess's
        // PATH and turn the spawn into MissingBinary.
        opts.path_override = Some("/usr/bin:/bin".into());
        opts
    }

    /// Cover the three exit-classification branches in `run_debug`'s
    /// non-zero path: "No results found" → typed NoResults; "Episode
    /// not released" → keyed Scraper; any other stderr → catch-all
    /// Scraper. Bundled into one test so the parallel
    /// `locate_ani_cli_*` test can't race-clear $PATH between sub-
    /// cases and turn a spawn into MissingBinary.
    #[tokio::test]
    async fn run_debug_classifies_nonzero_exits_by_stderr_pattern() {
        let _guard = ENV_LOCK.lock().await;
        let (_td1, p1) = stub_ani_cli("No results found", 1);
        let r1 = run_debug(&debug_opts(p1), "any", "1", "best", "sub", 1).await;
        assert!(matches!(r1, Err(AniError::NoResults)), "got: {r1:?}");

        let (_td2, p2) = stub_ani_cli("Episode not released", 1);
        let r2 = run_debug(&debug_opts(p2), "any", "999", "best", "sub", 1).await;
        assert!(matches!(r2, Err(AniError::Scraper { .. })), "got: {r2:?}");

        let (_td3, p3) = stub_ani_cli("could not resolve host", 6);
        let r3 = run_debug(&debug_opts(p3), "any", "1", "best", "sub", 1).await;
        assert!(matches!(r3, Err(AniError::Scraper { .. })), "got: {r3:?}");
    }

    /// Same exit-classification logic in the streaming variant — covers
    /// the SSE play endpoint's error paths.
    #[tokio::test]
    async fn run_debug_streaming_classifies_nonzero_exits_by_stderr_pattern() {
        let _guard = ENV_LOCK.lock().await;
        let (_td1, p1) = stub_ani_cli("No results found", 1);
        let r1 = run_debug_streaming(&debug_opts(p1), "any", "1", "best", "sub", 1, |_| {}).await;
        assert!(matches!(r1, Err(AniError::NoResults)), "got: {r1:?}");

        let (_td2, p2) = stub_ani_cli("Episode not released", 1);
        let r2 = run_debug_streaming(&debug_opts(p2), "any", "1", "best", "sub", 1, |_| {}).await;
        assert!(matches!(r2, Err(AniError::Scraper { .. })), "got: {r2:?}");
    }
}
