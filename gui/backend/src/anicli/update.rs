//! ani-cli script self-update infrastructure (red commit — stubs).
//!
//! Tests live below; the four pure helpers are stubbed with `todo!()`
//! so the suite fails. The matching green commit replaces the stubs
//! with real implementations.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

pub const SETTING_KEY_AUTO_UPDATE: &str = "auto_update_anicli";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateOutcome {
    pub status: UpdateStatus,
    pub stdout: String,
    pub stderr: String,
    #[serde(with = "rfc3339")]
    pub finished_at: SystemTime,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    NoChange,
    Updated,
    Failed,
}

pub fn should_update_now(_last: Option<SystemTime>, _ttl: Duration, _enabled: bool) -> bool {
    todo!("green commit")
}

pub fn resolve_anicli_path(_seed: &Path, _cache_dir: &Path) -> std::io::Result<PathBuf> {
    todo!("green commit")
}

pub fn reapply_lib_guard(_script_path: &Path) -> std::io::Result<()> {
    todo!("green commit")
}

pub async fn run_update(_script_path: &Path) -> UpdateOutcome {
    todo!("green commit")
}

mod rfc3339 {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(_t: &SystemTime, _s: S) -> Result<S::Ok, S::Error> {
        todo!("green commit")
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(_d: D) -> Result<SystemTime, D::Error> {
        todo!("green commit")
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
            .find("__ANI_CLI_LIB__")
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
        assert!(body.contains("__ANI_CLI_LIB__"), "guard appended");
    }

    // ── UpdateOutcome (de)serialisation ─────────────────────────────────

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
