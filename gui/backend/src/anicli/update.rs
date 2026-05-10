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
//! 2. **Reapply our patch.** The vendored script carries one local
//!    modification — the `__ANI_CLI_LIB__` source guard that lets the
//!    bats test suite source the script without executing it. `ani-cli
//!    -U` overwrites the file via `patch`, so we re-apply the guard
//!    after every successful update. See [`reapply_lib_guard`].
//!
//! 3. **Decide when to run.** Daily by default (24 h TTL on the cache
//!    file's mtime), gated by a settings toggle. See [`should_update_now`].
//!
//! 4. **Run `-U`.** Spawn `bash <cached_script> -U`, capture stdout +
//!    stderr, classify the outcome. Always run the patch reapply
//!    afterwards regardless of whether `-U` reported a change — the
//!    upstream might have shifted the patch's anchor lines without
//!    touching ours. See [`run_update`] / [`UpdateOutcome`].

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

/// Pick the staleness anchor [`should_update_now`] should compare
/// against. Returns the *later* of the persisted outcome's
/// `finished_at` and the script file's mtime — both are tracked
/// because they cover different scenarios:
///
/// - **Fresh install**: no outcome has been written yet, so we fall
///   back to the script mtime (set when the seed was copied to cache
///   on first boot). This stops a brand-new install from immediately
///   running `-U` while the seed is still warm.
/// - **NoChange-after-update**: `-U` exits without writing the
///   script, so its mtime stays old; without the outcome timestamp
///   we'd re-curl GitHub on every reopen for the rest of the day.
///   The outcome is written on every run (including NoChange) and
///   makes the TTL gate stick.
#[must_use]
pub fn last_run_anchor(
    outcome_finished_at: Option<SystemTime>,
    script_mtime: Option<SystemTime>,
) -> Option<SystemTime> {
    match (outcome_finished_at, script_mtime) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// Decide whether to spawn an `-U` run on backend boot.
///
/// `last` is the script's mtime (or `None` if never updated); `ttl` is
/// the staleness window from settings (default 24 h); `enabled` is the
/// per-user toggle. All three must concur.
pub fn should_update_now(last: Option<SystemTime>, ttl: Duration, enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    let Some(last) = last else {
        return true; // never updated — go
    };
    SystemTime::now()
        .duration_since(last)
        .map(|elapsed| elapsed >= ttl)
        .unwrap_or(false) // mtime in the future = clock skew, don't run
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

/// Re-apply the `__ANI_CLI_LIB__` source guard to `script_path` if it
/// isn't already there. Idempotent: running twice is a no-op.
///
/// The guard is a single line `[ -n "$__ANI_CLI_LIB__" ] && return 0`
/// appended near the bottom of the script (before `main "$@"`). It
/// lets the bats test loader pull the script in without executing
/// `main`. `-U` clobbers the file via `patch`, so we re-apply after
/// every successful run.
///
/// # Errors
/// Returns [`std::io::Error`] on read/write failure.
pub fn reapply_lib_guard(script_path: &Path) -> std::io::Result<()> {
    let contents = std::fs::read_to_string(script_path)?;
    if contents.contains(LIB_GUARD_MARKER) {
        return Ok(());
    }
    // Insert before the final `main "$@"` invocation. If we can't find
    // it, append at end — better to have the guard at the wrong place
    // than to lose it entirely.
    let guard_line = format!("[ -n \"${LIB_GUARD_MARKER}\" ] && return 0\n");
    let patched = match contents.rfind("\nmain \"$@\"") {
        Some(idx) => {
            let mut out = String::with_capacity(contents.len() + guard_line.len());
            out.push_str(&contents[..idx]);
            out.push('\n');
            out.push_str(&guard_line);
            out.push_str(&contents[idx + 1..]);
            out
        }
        None => format!("{contents}\n{guard_line}"),
    };
    std::fs::write(script_path, patched)?;
    Ok(())
}

/// Run `bash <script_path> -U` and classify the outcome by parsing the
/// upstream `update_script` function's output strings. The implementation
/// hands its caller the captured stdout + stderr verbatim so the
/// diagnostics page can render the user-facing log line.
///
/// # Errors
/// Never returns `Err`; spawn failures and non-zero exits are surfaced
/// as `UpdateStatus::Failed` with the error in `stderr`.
pub async fn run_update(script_path: &Path) -> UpdateOutcome {
    let started = std::time::Instant::now();
    let result = tokio::process::Command::new("bash")
        .arg(script_path)
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

/// On-disk filename (under `$XDG_STATE_HOME/ani-gui/`) for the latest
/// outcome. Written after every run; read by /diagnostics.
pub const OUTCOME_FILENAME: &str = "anicli-update.json";

/// Persist `outcome` to `<state_dir>/anicli-update.json`. Atomic via
/// `path.new` + rename so a partial write can't corrupt the file the
/// next boot reads.
///
/// # Errors
/// Returns [`std::io::Error`] on directory creation, write, or rename
/// failure.
pub fn write_outcome(state_dir: &Path, outcome: &UpdateOutcome) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let target = state_dir.join(OUTCOME_FILENAME);
    let tmp = state_dir.join(format!("{OUTCOME_FILENAME}.new"));
    let body = serde_json::to_string_pretty(outcome).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Read the latest persisted outcome. Returns `Ok(None)` when the
/// file doesn't exist (first boot, or never run); `Err` on I/O or
/// parse failure.
pub fn read_outcome(state_dir: &Path) -> std::io::Result<Option<UpdateOutcome>> {
    let target = state_dir.join(OUTCOME_FILENAME);
    if !target.is_file() {
        return Ok(None);
    }
    let body = std::fs::read_to_string(&target)?;
    let outcome: UpdateOutcome = serde_json::from_str(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(outcome))
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

    // ── should_update_now ────────────────────────────────────────────────

    #[test]
    fn should_update_now_returns_false_when_disabled() {
        assert!(!should_update_now(None, Duration::from_secs(60), false));
    }

    #[test]
    fn should_update_now_returns_true_when_never_updated_and_enabled() {
        assert!(should_update_now(None, Duration::from_secs(60), true));
    }

    #[test]
    fn should_update_now_returns_true_when_older_than_ttl() {
        let two_days_ago = SystemTime::now() - Duration::from_secs(2 * 86_400);
        assert!(should_update_now(
            Some(two_days_ago),
            Duration::from_secs(86_400),
            true
        ));
    }

    #[test]
    fn should_update_now_returns_false_when_within_ttl() {
        let just_now = SystemTime::now() - Duration::from_secs(60);
        assert!(!should_update_now(
            Some(just_now),
            Duration::from_secs(86_400),
            true
        ));
    }

    #[test]
    fn should_update_now_returns_false_when_mtime_in_future() {
        let future = SystemTime::now() + Duration::from_secs(86_400);
        assert!(!should_update_now(
            Some(future),
            Duration::from_secs(60),
            true
        ));
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

    // ── reapply_lib_guard ───────────────────────────────────────────────

    #[test]
    fn reapply_lib_guard_inserts_before_main_invocation() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        std::fs::write(&path, "#!/bin/sh\nfoo() { :; }\n\nmain \"$@\"\n").unwrap();
        reapply_lib_guard(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        let guard_pos = body
            .find(LIB_GUARD_MARKER)
            .expect("guard marker present after reapply");
        let main_pos = body.find("main \"$@\"").unwrap();
        assert!(guard_pos < main_pos, "guard must precede main invocation");
    }

    #[test]
    fn reapply_lib_guard_is_idempotent() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        std::fs::write(&path, "#!/bin/sh\nmain \"$@\"\n").unwrap();
        reapply_lib_guard(&path).unwrap();
        let after_first = std::fs::read_to_string(&path).unwrap();
        reapply_lib_guard(&path).unwrap();
        let after_second = std::fs::read_to_string(&path).unwrap();
        assert_eq!(after_first, after_second, "second reapply must be a no-op");
    }

    #[test]
    fn reapply_lib_guard_appends_when_no_main_anchor() {
        let dir = tmpdir();
        let path = dir.path().join("ani-cli");
        std::fs::write(&path, "#!/bin/sh\necho hello\n").unwrap();
        reapply_lib_guard(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains(LIB_GUARD_MARKER), "guard appended");
    }

    // ── UpdateOutcome (de)serialisation ─────────────────────────────────

    // ── last_run_anchor ────────────────────────────────────────────────

    #[test]
    fn last_run_anchor_returns_none_when_both_missing() {
        assert_eq!(last_run_anchor(None, None), None);
    }

    #[test]
    fn last_run_anchor_falls_back_to_mtime_when_no_outcome() {
        // Fresh-install case: only the script's mtime is known
        // because no `-U` has run yet.
        let mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(last_run_anchor(None, Some(mtime)), Some(mtime));
    }

    #[test]
    fn last_run_anchor_uses_outcome_when_newer_than_mtime() {
        // NoChange-after-update case: `-U` ran, wrote the outcome,
        // but didn't touch the script. The outcome timestamp is the
        // authoritative "last attempt".
        let old_mtime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let recent_outcome = UNIX_EPOCH + Duration::from_secs(1_700_086_400);
        assert_eq!(
            last_run_anchor(Some(recent_outcome), Some(old_mtime)),
            Some(recent_outcome)
        );
    }

    #[test]
    fn last_run_anchor_uses_mtime_when_newer_than_outcome() {
        // Defensive: a manual `bash ani-cli -U` outside our binary
        // could touch the file later than our last persisted run.
        // Pick the genuinely-most-recent timestamp.
        let old_outcome = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let recent_mtime = UNIX_EPOCH + Duration::from_secs(1_700_086_400);
        assert_eq!(
            last_run_anchor(Some(old_outcome), Some(recent_mtime)),
            Some(recent_mtime)
        );
    }

    // ── persistence ────────────────────────────────────────────────────

    #[test]
    fn write_then_read_outcome_round_trips() {
        let dir = tmpdir();
        let outcome = UpdateOutcome {
            status: UpdateStatus::Updated,
            stdout: "ok\n".into(),
            stderr: String::new(),
            finished_at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            duration_ms: 42,
        };
        write_outcome(dir.path(), &outcome).unwrap();
        let back = read_outcome(dir.path()).unwrap().unwrap();
        assert_eq!(back, outcome);
    }

    #[test]
    fn read_outcome_returns_none_when_file_missing() {
        let dir = tmpdir();
        assert!(read_outcome(dir.path()).unwrap().is_none());
    }

    #[test]
    fn write_outcome_creates_state_dir_when_missing() {
        let dir = tmpdir();
        let nested = dir.path().join("a/b/c");
        let outcome = UpdateOutcome {
            status: UpdateStatus::NoChange,
            stdout: String::new(),
            stderr: String::new(),
            finished_at: UNIX_EPOCH,
            duration_ms: 0,
        };
        write_outcome(&nested, &outcome).unwrap();
        assert!(nested.join(OUTCOME_FILENAME).is_file());
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
