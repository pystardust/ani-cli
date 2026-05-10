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
/// candidate paths to probe for `bash.exe` on Windows.
#[must_use]
pub fn windows_bash_candidate_paths(_env: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    // Stub — the real implementation lives in the green commit.
    Vec::new()
}

/// Probe filesystem for the first existing path in `candidates`.
#[must_use]
pub fn pick_first_existing(
    _candidates: &[PathBuf],
    _is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    // Stub — the real implementation lives in the green commit.
    None
}

/// On Windows, build `bash.exe <ani_cli_path>`. On Unix, build
/// `<ani_cli_path>` directly. Centralised so every spawn site does
/// the same thing.
pub fn build_anicli_command(
    _ani_cli_path: &Path,
    _bash_path: Option<&Path>,
) -> tokio::process::Command {
    // Stub — the real implementation lives in the green commit.
    tokio::process::Command::new("__bash_stub_unimplemented__")
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
