//! ani-cli script self-update infrastructure.
//!
//! `ani-cli` is a single Bash script whose scraping logic drifts daily —
//! upstream pushes hotfixes when allmanga changes its API. The bundled
//! copy in `process.resourcesPath` is a snapshot; it goes stale fast.
//!
//! The strategy:
//!
//! 1. **Resolve to a writable copy.** AppImage's resources dir is
//!    read-only. On first launch we copy the seed (the bundled script)
//!    to `$XDG_CACHE_HOME/ani-gui/ani-cli` and use that path everywhere.
//!    See [`resolve_anicli_path`].
//!
//! 2. **Strip the test-loader guard.** The seed in the repo carries
//!    a single `__ANI_CLI_LIB__` source-guard line so the bats test
//!    loader can source the script without executing it. The runtime
//!    cache invokes via `bash <path>` and never sources, so the
//!    guard is dead code there — and worse, it's the one line that
//!    always differs from upstream master, which would make `-U`
//!    report `Updated` on every single boot in a perpetual remove-
//!    then-reapply cycle. See [`strip_lib_guard`].
//!
//! 3. **Run `-U` on every boot.** Gated only by the per-user
//!    settings toggle. The task is spawned in the background after
//!    the listener binds, so app launch is unaffected. See
//!    [`run_update`] / [`UpdateOutcome`].

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Marker for our one carried patch — gates `source ani-cli` so the
/// bats test loader can pull the script in without running `main`.
const LIB_GUARD_MARKER: &str = "__ANI_CLI_LIB__";

/// What the user-facing settings toggle is called when serialized — the
/// frontend mirrors it. `auto_update_anicli` reads as the active state
/// rather than the inverted "disable" polarity used elsewhere; users
/// expect this to default ON.
pub const SETTING_KEY_AUTO_UPDATE: &str = "auto_update_anicli";

/// Outcome of a single `-U` run. Persisted so /diagnostics can render
/// the last attempt's status, output, and timestamp without keeping a
/// long-lived in-memory log.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateOutcome {
    /// Classified outcome of the run; see [`UpdateStatus`].
    pub status: UpdateStatus,
    /// Captured stdout of `bash <script> -U`. The diagnostics page
    /// renders this verbatim — short enough (one or two lines) that
    /// pretty-printing isn't worth it.
    pub stdout: String,
    /// Captured stderr. Empty on success; carries the upstream
    /// "Connection error" / "Can't update" text on failure.
    pub stderr: String,
    /// Wall-clock at the moment the run finished. Serialised as RFC3339.
    #[serde(with = "rfc3339")]
    pub finished_at: SystemTime,
    /// How long the run took, in milliseconds.
    pub duration_ms: u64,
}

/// Three-way status: `NoChange` and `Updated` are both "ran without
/// erroring"; `Failed` covers spawn failure, non-zero exit, and any
/// other transport error. The detail goes in [`UpdateOutcome::stderr`].
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// Upstream `master` matches the cached script — `-U` exited 0
    /// with the "Script is up to date" line.
    NoChange,
    /// Upstream had drift; the cache was patched in place.
    Updated,
    /// Spawn failure, non-zero exit, or unparseable output.
    Failed,
}

/// Materialise a writable script path from a seed (the bundled script)
/// plus the cache directory. Copies the seed to `<cache_dir>/ani-cli`
/// when the target is missing; returns the cache path either way.
///
/// # Errors
/// Returns an [`std::io::Error`] when the seed is missing, the cache
/// directory can't be created, or the copy fails. The cache path is
/// only returned on a healthy state.
pub fn resolve_anicli_path(seed: &Path, cache_dir: &Path) -> std::io::Result<PathBuf> {
    let target = cache_dir.join("ani-cli");
    if target.is_file() {
        return Ok(target);
    }
    if !seed.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("seed script missing at {}", seed.display()),
        ));
    }
    std::fs::create_dir_all(cache_dir)?;
    std::fs::copy(seed, &target)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target, perms)?;
    }
    Ok(target)
}

/// Remove the `__ANI_CLI_LIB__` source-guard line from `script_path`
/// if present. Idempotent: a no-op when the line isn't there.
///
/// The guard exists in the repo's seed script for the bats test
/// loader, which sources `ani-cli` directly. The runtime cache copy
/// invokes the script via `bash <path>`, where the guard is dead
/// code. Carrying it forward is actively harmful: it's the one line
/// that always differs between our cache and upstream master, which
/// makes `ani-cli -U` report `Updated` on every single boot (the
/// patch keeps removing the guard and we'd have to re-apply it,
/// burning a bash + patch + write cycle for nothing).
///
/// # Errors
/// Returns [`std::io::Error`] on read/write failure.
pub fn strip_lib_guard(script_path: &Path) -> std::io::Result<()> {
    let contents = std::fs::read_to_string(script_path)?;
    if !contents.contains(LIB_GUARD_MARKER) {
        return Ok(());
    }
    let stripped: String = contents
        .lines()
        .filter(|line| !line.contains(LIB_GUARD_MARKER))
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(script_path, stripped)?;
    Ok(())
}

/// Run `bash <script_path> -U` and classify the outcome by parsing the
/// upstream `update_script` function's output strings. The implementation
/// hands its caller the captured stdout + stderr verbatim so the
/// diagnostics page can render the user-facing log line.
///
/// `bash_path` is required on Windows (resolved via
/// [`crate::anicli::bash::locate_bash`] at startup) and ignored on
/// Unix where bash is found on PATH naturally.
///
/// # Errors
/// Never returns `Err`; spawn failures and non-zero exits are surfaced
/// as `UpdateStatus::Failed` with the error in `stderr`.
pub async fn run_update(script_path: &Path, bash_path: Option<&Path>) -> UpdateOutcome {
    let started = std::time::Instant::now();
    let mut cmd = match bash_path {
        // Windows: bash.exe resolved at startup via locate_bash. Apply
        // CREATE_NO_WINDOW so the spawn doesn't flash a console
        // window every -U run.
        Some(bash) => {
            #[allow(unused_mut)] // mut needed only on Windows for creation_flags
            let mut c = tokio::process::Command::new(bash);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                c.creation_flags(CREATE_NO_WINDOW);
            }
            c
        }
        // Unix: bash on PATH (today's behaviour). We deliberately
        // invoke bash even though the script's shebang would
        // resolve to /bin/sh — Ubuntu's /bin/sh is dash and has
        // subtle string-handling differences from bash.
        None => tokio::process::Command::new("bash"),
    };
    cmd.arg(script_path);
    let result = cmd
        .arg("-U")
        // Don't carry forward the user's $TERM / colour env; -U writes
        // to stdout in plain ASCII.
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;
    let duration_ms = started.elapsed().as_millis() as u64;
    let finished_at = SystemTime::now();
    match result {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            let status = if !out.status.success() {
                UpdateStatus::Failed
            } else if stdout.contains("up to date") {
                UpdateStatus::NoChange
            } else if stdout.contains("updated") {
                UpdateStatus::Updated
            } else {
                // Defensive: exit 0 + unrecognised output. Treat as no
                // change so we don't reapply the guard unnecessarily,
                // but surface the raw output.
                UpdateStatus::NoChange
            };
            UpdateOutcome {
                status,
                stdout,
                stderr,
                finished_at,
                duration_ms,
            }
        }
        Err(e) => UpdateOutcome {
            status: UpdateStatus::Failed,
            stdout: String::new(),
            stderr: format!("spawn error: {e}"),
            finished_at,
            duration_ms,
        },
    }
}

/// On-disk filename (under `$XDG_STATE_HOME/ani-gui/`) for the
/// rolling log of recent outcomes. Stored as a JSON array, latest
/// entry first, capped at [`OUTCOME_LOG_CAP`] entries.
pub const OUTCOMES_LOG_FILENAME: &str = "anicli-update-log.json";

/// How many past run outcomes we keep on disk. Past this point the
/// oldest entries roll off; the on-disk file stays small (~tens of
/// KB even at the cap).
pub const OUTCOME_LOG_CAP: usize = 100;

/// Append `outcome` as the newest entry in the rolling log at
/// `<state_dir>/anicli-update-log.json`. Caps the log at
/// [`OUTCOME_LOG_CAP`] entries; older entries roll off. Atomic via
/// `path.new` + rename so a partial write can't corrupt the next
/// boot's read.
///
/// A corrupt or unreadable existing log is treated as empty — the
/// new entry replaces the file rather than blocking on a parse
/// error the user can't act on.
///
/// # Errors
/// Returns [`std::io::Error`] on directory creation, write, or
/// rename failure.
pub fn append_outcome(state_dir: &Path, outcome: &UpdateOutcome) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let mut existing = read_outcomes(state_dir).unwrap_or_default();
    existing.insert(0, outcome.clone());
    if existing.len() > OUTCOME_LOG_CAP {
        existing.truncate(OUTCOME_LOG_CAP);
    }
    let target = state_dir.join(OUTCOMES_LOG_FILENAME);
    let tmp = state_dir.join(format!("{OUTCOMES_LOG_FILENAME}.new"));
    let body = serde_json::to_string_pretty(&existing).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Read the persisted log of recent outcomes, latest first. Returns
/// `Ok(vec![])` when the file doesn't exist (first boot, never run).
///
/// # Errors
/// I/O failure on read; parse failure on a corrupt JSON file.
pub fn read_outcomes(state_dir: &Path) -> std::io::Result<Vec<UpdateOutcome>> {
    let target = state_dir.join(OUTCOMES_LOG_FILENAME);
    if !target.is_file() {
        return Ok(Vec::new());
    }
    let body = std::fs::read_to_string(&target)?;
    let log: Vec<UpdateOutcome> = serde_json::from_str(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(log)
}

/// RFC3339 (de)serialisation for `SystemTime`. Stored as a string so
/// the on-disk JSON is human-readable in /diagnostics' raw view.
mod rfc3339 {
    use super::*;
    use serde::{de::Error as _, Deserializer, Serializer};
    use std::time::UNIX_EPOCH;

    pub fn serialize<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
        let secs = t
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Avoid pulling chrono in just for this — emit a Z-suffixed
        // ISO-8601 timestamp computed from the epoch seconds.
        s.serialize_str(&format_epoch_secs(secs))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
        use serde::Deserialize as _;
        let raw = String::deserialize(d)?;
        parse_iso8601_z(&raw)
            .map(|secs| UNIX_EPOCH + Duration::from_secs(secs))
            .ok_or_else(|| D::Error::custom(format!("invalid timestamp: {raw}")))
    }

    /// Format `secs` since UNIX epoch as `YYYY-MM-DDTHH:MM:SSZ`. Pure
    /// proleptic Gregorian arithmetic — good for any post-1970 instant.
    fn format_epoch_secs(secs: u64) -> String {
        let days = (secs / 86_400) as i64;
        let rem = (secs % 86_400) as u32;
        let (y, m, d) = days_to_ymd(days);
        let h = rem / 3600;
        let mi = (rem % 3600) / 60;
        let s = rem % 60;
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    }

    fn parse_iso8601_z(raw: &str) -> Option<u64> {
        // Parse `YYYY-MM-DDTHH:MM:SSZ` exactly. Anything else returns None.
        let bytes = raw.as_bytes();
        if bytes.len() != 20 || bytes[19] != b'Z' || bytes[10] != b'T' {
            return None;
        }
        let y: i64 = raw[0..4].parse().ok()?;
        let mo: u32 = raw[5..7].parse().ok()?;
        let d: u32 = raw[8..10].parse().ok()?;
        let h: u64 = raw[11..13].parse().ok()?;
        let mi: u64 = raw[14..16].parse().ok()?;
        let s: u64 = raw[17..19].parse().ok()?;
        let days = ymd_to_days(y, mo, d)?;
        Some((days as u64) * 86_400 + h * 3600 + mi * 60 + s)
    }

    /// Convert a count of days since 1970-01-01 to a (year, month, day)
    /// triple. Algorithm by Howard Hinnant — works for any int64 day
    /// count.
    fn days_to_ymd(days: i64) -> (i32, u32, u32) {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
        let y = if m <= 2 { y + 1 } else { y } as i32;
        (y, m, d)
    }

    fn ymd_to_days(y: i64, m: u32, d: u32) -> Option<i64> {
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400) as u64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + (d - 1) as u64;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Some(era * 146_097 + doe as i64 - 719_468)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn tmpdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tmpdir")
    }

    // ── resolve_anicli_path ─────────────────────────────────────────────

    #[test]
    fn resolve_copies_seed_when_cache_missing() {
        let dir = tmpdir();
        let seed = dir.path().join("seed-ani-cli");
        std::fs::write(&seed, "#!/bin/sh\necho seed\n").unwrap();
        let cache = dir.path().join("cache");
        let resolved = resolve_anicli_path(&seed, &cache).expect("resolve");
        assert_eq!(resolved, cache.join("ani-cli"));
        let body = std::fs::read_to_string(&resolved).unwrap();
        assert!(body.contains("echo seed"));
    }

    #[test]
    fn resolve_returns_existing_cache_without_overwriting() {
        let dir = tmpdir();
        let seed = dir.path().join("seed-ani-cli");
        std::fs::write(&seed, "#!/bin/sh\necho seed\n").unwrap();
        let cache = dir.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let cached = cache.join("ani-cli");
        std::fs::write(&cached, "#!/bin/sh\necho cached\n").unwrap();
        let resolved = resolve_anicli_path(&seed, &cache).expect("resolve");
        let body = std::fs::read_to_string(&resolved).unwrap();
        assert!(body.contains("echo cached"), "must not overwrite cache");
    }

    #[test]
    fn resolve_errors_when_seed_missing_and_cache_missing() {
        let dir = tmpdir();
        let seed = dir.path().join("nope");
        let cache = dir.path().join("cache");
        let err = resolve_anicli_path(&seed, &cache).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_marks_copy_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmpdir();
        let seed = dir.path().join("seed-ani-cli");
        std::fs::write(&seed, "#!/bin/sh\n").unwrap();
        let cache = dir.path().join("cache");
        let resolved = resolve_anicli_path(&seed, &cache).unwrap();
        let mode = std::fs::metadata(&resolved).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "copy must be executable: {mode:o}");
    }

    // ── strip_lib_guard ─────────────────────────────────────────────────

    #[test]
    fn strip_lib_guard_removes_the_marker_line() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        std::fs::write(
            &path,
            "#!/bin/sh\nfoo() { :; }\n[ -n \"$__ANI_CLI_LIB__\" ] && return 0\nmain \"$@\"\n",
        )
        .unwrap();
        strip_lib_guard(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(!body.contains(LIB_GUARD_MARKER), "guard line removed");
        assert!(body.contains("foo() { :; }"), "other lines preserved");
        assert!(body.contains("main \"$@\""), "main invocation preserved");
    }

    #[test]
    fn strip_lib_guard_is_a_noop_when_marker_absent() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        let original = "#!/bin/sh\necho hello\nmain \"$@\"\n";
        std::fs::write(&path, original).unwrap();
        strip_lib_guard(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, original, "no-op when guard isn't present");
    }

    #[test]
    fn strip_lib_guard_is_idempotent() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        std::fs::write(
            &path,
            "#!/bin/sh\n[ -n \"$__ANI_CLI_LIB__\" ] && return 0\necho hi\n",
        )
        .unwrap();
        strip_lib_guard(&path).unwrap();
        let after_first = std::fs::read_to_string(&path).unwrap();
        strip_lib_guard(&path).unwrap();
        let after_second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(after_first, after_second, "second strip must be a no-op");
    }

    // ── UpdateOutcome (de)serialisation ─────────────────────────────────

    // ── persistence ────────────────────────────────────────────────────

    fn make_outcome(epoch_secs: u64, status: UpdateStatus) -> UpdateOutcome {
        UpdateOutcome {
            status,
            stdout: format!("run @ {epoch_secs}\n"),
            stderr: String::new(),
            finished_at: UNIX_EPOCH + Duration::from_secs(epoch_secs),
            duration_ms: 42,
        }
    }

    #[test]
    fn append_outcome_round_trips_a_single_entry() {
        let dir = tmpdir();
        let outcome = make_outcome(1_700_000_000, UpdateStatus::Updated);
        append_outcome(dir.path(), &outcome).unwrap();
        let log = read_outcomes(dir.path()).unwrap();
        assert_eq!(log, vec![outcome]);
    }

    #[test]
    fn read_outcomes_returns_empty_when_file_missing() {
        let dir = tmpdir();
        assert_eq!(read_outcomes(dir.path()).unwrap(), Vec::new());
    }

    #[test]
    fn append_outcome_creates_state_dir_when_missing() {
        let dir = tmpdir();
        let nested = dir.path().join("a/b/c");
        append_outcome(&nested, &make_outcome(1, UpdateStatus::NoChange)).unwrap();
        assert!(nested.join(OUTCOMES_LOG_FILENAME).is_file());
    }

    #[test]
    fn append_outcome_pushes_newest_to_the_front() {
        let dir = tmpdir();
        let first = make_outcome(1, UpdateStatus::Updated);
        let second = make_outcome(2, UpdateStatus::NoChange);
        append_outcome(dir.path(), &first).unwrap();
        append_outcome(dir.path(), &second).unwrap();
        let log = read_outcomes(dir.path()).unwrap();
        assert_eq!(log, vec![second, first]);
    }

    #[test]
    fn append_outcome_caps_at_cap_size() {
        let dir = tmpdir();
        for i in 0..(OUTCOME_LOG_CAP as u64 + 5) {
            append_outcome(dir.path(), &make_outcome(i, UpdateStatus::NoChange)).unwrap();
        }
        let log = read_outcomes(dir.path()).unwrap();
        assert_eq!(log.len(), OUTCOME_LOG_CAP);
        // Newest first; the most recent run is at the head.
        assert_eq!(
            log[0].finished_at,
            UNIX_EPOCH + Duration::from_secs(OUTCOME_LOG_CAP as u64 + 4)
        );
    }

    #[test]
    fn update_outcome_round_trips_through_json() {
        let outcome = UpdateOutcome {
            status: UpdateStatus::Updated,
            stdout: "Script has been updated\n".to_string(),
            stderr: String::new(),
            finished_at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            duration_ms: 1234,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"updated\""));
        assert!(json.contains("2023-11-14T22:13:20Z"));
        let back: UpdateOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(back, outcome);
    }
}
