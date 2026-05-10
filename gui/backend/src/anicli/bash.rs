//! Locate `bash.exe` on Windows so the backend can drive the POSIX
//! `ani-cli` script without forcing the user through a manual scoop +
//! Windows Terminal setup. The pattern matches what the README
//! advertises for CLI users — Git for Windows in `Program Files`, or
//! a scoop install under `%USERPROFILE%\scoop\apps\git\current\bin\` —
//! plus a PATH probe so users who exposed `bash` themselves "just
//! work."
//!
//! On Unix the entire module is inert: `build_anicli_command` returns
//! the Command unchanged and `locate_bash` is never called.

use std::path::{Path, PathBuf};

/// Pure: given a closure that resolves env vars, return the ordered
/// candidate paths to probe for `bash.exe` on Windows. Order matters
/// — the caller takes the first one that exists on disk.
///
/// Probe order rationale:
///   1. `%ProgramFiles%\Git\bin\bash.exe` — the default Git for
///      Windows install path; covers `winget install Git.Git`,
///      manual installer, most enterprise images.
///   2. `%ProgramFiles(x86)%\Git\bin\bash.exe` — 32-bit Git for
///      Windows on a 64-bit OS (rare, but cheap to check).
///   3. `%USERPROFILE%\scoop\apps\git\current\bin\bash.exe` — the
///      scoop-recommended path the upstream ani-cli README walks
///      users through.
///
/// Falls back to canonical literals when `ProgramFiles` / `(x86)`
/// are absent so the list is never empty on a freshly-imaged machine
/// where the user's shell hasn't populated env yet. Omits the scoop
/// entry when `USERPROFILE` is absent because we can't construct a
/// useful path without it.
#[must_use]
pub fn windows_bash_candidate_paths(env: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(3);

    let pf = env("ProgramFiles").unwrap_or_else(|| r"C:\Program Files".to_string());
    out.push(PathBuf::from(format!(r"{pf}\Git\bin\bash.exe")));

    let pf86 = env("ProgramFiles(x86)").unwrap_or_else(|| r"C:\Program Files (x86)".to_string());
    out.push(PathBuf::from(format!(r"{pf86}\Git\bin\bash.exe")));

    if let Some(home) = env("USERPROFILE") {
        out.push(PathBuf::from(format!(
            r"{home}\scoop\apps\git\current\bin\bash.exe"
        )));
    }
    out
}

/// Probe filesystem for the first existing path in `candidates`.
/// Pure with respect to `is_file`, which the caller supplies — tests
/// inject a closure to drive specific branches without touching real
/// disk; production callers pass `Path::is_file` directly.
#[must_use]
pub fn pick_first_existing(
    candidates: &[PathBuf],
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    candidates.iter().find(|p| is_file(p)).cloned()
}

/// Resolve `bash.exe` at startup on Windows.
///
/// 1. PATH — covers users who added Git Bash to PATH or installed
///    via scoop's shim
/// 2. Candidate paths from [`windows_bash_candidate_paths`]
///
/// Returns `None` only when the user has no Git for Windows install
/// reachable; the caller surfaces [`crate::error::AniError::BashMissing`]
/// to the frontend with a one-link install pointer.
///
/// On Unix this function isn't called — the spawn path uses
/// [`build_anicli_command`] which is a noop there.
#[cfg(windows)]
#[must_use]
pub fn locate_bash() -> Option<PathBuf> {
    if let Some(p) = crate::anicli::process::find_in_path("bash.exe") {
        return Some(p);
    }
    let candidates = windows_bash_candidate_paths(|k| std::env::var(k).ok());
    pick_first_existing(&candidates, |p| p.is_file())
}

/// On Windows, build `bash.exe <ani_cli_path>` and apply the
/// `CREATE_NO_WINDOW` flag so spawning doesn't briefly flash a
/// console window (bash.exe is a console-subsystem binary; without
/// the flag, GUI apps that spawn it get a visible flicker on every
/// invocation). On Unix, build `<ani_cli_path>` directly.
///
/// `bash_path` is required on Windows and ignored on Unix. Pass
/// `None` on Unix; pass `Some(resolved)` on Windows where the
/// caller has already run [`locate_bash`].
pub fn build_anicli_command(
    ani_cli_path: &Path,
    bash_path: Option<&Path>,
) -> tokio::process::Command {
    #[cfg(windows)]
    {
        // creation_flags is a direct method on tokio::process::Command
        // under cfg(windows); no extension trait import needed.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let bash = bash_path.expect(
            "bash_path is required on Windows — call locate_bash at startup and surface BashMissing if it returns None",
        );
        let mut cmd = tokio::process::Command::new(bash);
        cmd.arg(ani_cli_path).creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        let _ = bash_path;
        tokio::process::Command::new(ani_cli_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from<'a>(map: &'a HashMap<&'a str, &'a str>) -> impl Fn(&str) -> Option<String> + 'a {
        move |k: &str| map.get(k).map(|v| (*v).to_string())
    }

    #[test]
    fn candidate_paths_uses_program_files_when_set() {
        let env = HashMap::from([
            ("ProgramFiles", r"D:\Apps"),
            ("ProgramFiles(x86)", r"D:\Apps86"),
            ("USERPROFILE", r"C:\Users\me"),
        ]);
        let v = windows_bash_candidate_paths(env_from(&env));
        assert_eq!(
            v,
            vec![
                PathBuf::from(r"D:\Apps\Git\bin\bash.exe"),
                PathBuf::from(r"D:\Apps86\Git\bin\bash.exe"),
                PathBuf::from(r"C:\Users\me\scoop\apps\git\current\bin\bash.exe"),
            ]
        );
    }

    #[test]
    fn candidate_paths_falls_back_to_literals_when_program_files_missing() {
        // First-launch on a fresh image: we still produce a sensible
        // list rather than emitting nothing.
        let env = HashMap::from([("USERPROFILE", r"C:\Users\me")]);
        let v = windows_bash_candidate_paths(env_from(&env));
        assert_eq!(
            v,
            vec![
                PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"),
                PathBuf::from(r"C:\Users\me\scoop\apps\git\current\bin\bash.exe"),
            ]
        );
    }

    #[test]
    fn candidate_paths_omits_scoop_when_userprofile_missing() {
        // Scoop installs under USERPROFILE — without it, we can't
        // construct the path. Keep the Program Files entries; that's
        // still better than an empty list.
        let env: HashMap<&str, &str> = HashMap::new();
        let v = windows_bash_candidate_paths(env_from(&env));
        assert_eq!(
            v,
            vec![
                PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"),
            ]
        );
    }

    #[test]
    fn pick_first_existing_returns_first_match() {
        let candidates = vec![
            PathBuf::from("/no/a"),
            PathBuf::from("/yes/b"),
            PathBuf::from("/yes/c"),
        ];
        let got = pick_first_existing(&candidates, |p| p.starts_with("/yes"));
        assert_eq!(got, Some(PathBuf::from("/yes/b")));
    }

    #[test]
    fn pick_first_existing_returns_none_when_none_exist() {
        let candidates = vec![PathBuf::from("/no/a"), PathBuf::from("/no/b")];
        let got = pick_first_existing(&candidates, |_| false);
        assert!(got.is_none());
    }

    #[test]
    fn pick_first_existing_handles_empty_candidate_list() {
        let got = pick_first_existing(&[], |_| true);
        assert!(got.is_none());
    }

    /// On Unix `build_anicli_command` ignores `bash_path` and runs the
    /// script directly. The test asserts the program name matches the
    /// script path so a future refactor that accidentally wraps Linux
    /// with bash gets caught.
    #[cfg(not(windows))]
    #[test]
    fn build_anicli_command_on_unix_runs_script_directly() {
        let cmd = build_anicli_command(Path::new("/usr/local/bin/ani-cli"), None);
        let std_cmd = cmd.as_std();
        assert_eq!(
            std_cmd.get_program(),
            std::ffi::OsStr::new("/usr/local/bin/ani-cli")
        );
        assert!(
            std_cmd.get_args().count() == 0,
            "no args injected on Unix; the script is the program"
        );
    }
}
