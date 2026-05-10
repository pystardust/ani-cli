//! PATH composition for `ani-cli` subprocess spawns.
//!
//! On Windows we ship a `bin/` directory next to the backend binary
//! containing `fzf.exe` (and any future POSIX-side ani-cli deps that
//! Git for Windows doesn't bundle). The script's `command -v fzf`
//! must resolve to that bundled copy before the system PATH, so we
//! prepend the bundled dir at every spawn site.
//!
//! This module exposes a single pure function that the spawn sites
//! call instead of building the PATH string inline. Pure (no env or
//! filesystem reads) so tests can drive every branch deterministically.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::error::{AniError, Result};

/// Default PATH used when neither the inherited env nor a test
/// override provides one. Matches the previous inline literal in
/// `process.rs` so behaviour is unchanged on a freshly-cleared env.
const FALLBACK_PATH: &str = "/usr/bin:/bin";

/// Compose the PATH env var for an ani-cli spawn.
///
/// Order of components in the returned value (platform-correct
/// separator via [`std::env::join_paths`]):
///
/// 1. `bundled_bin` — if provided, prepended so the bundled fzf wins
///    over any system install.
/// 2. `path_override` — wins over the inherited PATH when set
///    (tests inject this to put a curl shim ahead of system bins).
/// 3. `inherited` — the parent process's PATH, normally
///    `std::env::var_os("PATH")`.
/// 4. [`FALLBACK_PATH`] — last-ditch when none of the above are set.
///
/// Pure: no env or filesystem reads. Caller passes everything in.
#[must_use]
pub fn compose_anicli_path(
    bundled_bin: Option<&Path>,
    path_override: Option<&str>,
    inherited: Option<&OsStr>,
) -> OsString {
    let base: OsString = match path_override {
        Some(o) => OsString::from(o),
        None => match inherited {
            Some(p) => p.to_os_string(),
            None => OsString::from(FALLBACK_PATH),
        },
    };

    let mut parts: Vec<PathBuf> = Vec::new();
    if let Some(b) = bundled_bin {
        parts.push(b.to_path_buf());
    }
    for p in std::env::split_paths(&base) {
        parts.push(p);
    }

    // join_paths only fails if a component contains the platform's
    // path-list separator, which neither our bundled dir nor a
    // pre-split PATH should ever contain. Fall back to the un-prefixed
    // base string so a malformed bundled dir doesn't break spawns.
    std::env::join_paths(&parts).unwrap_or(base)
}

/// Locate `ffmpeg` inside a composed PATH string. Pure: caller
/// supplies the path-list and the executable check, so the test
/// suite can drive every branch without touching real disk.
///
/// Returns `Ok(())` when an executable matching the platform's
/// ffmpeg name (`ffmpeg.exe` on Windows, `ffmpeg` elsewhere) sits
/// in any of the path components. Otherwise returns
/// [`AniError::FfmpegMissing`] so the SSE download stream can
/// short-circuit before spawning ani-cli — surfacing the typed
/// error early lets the frontend render an actionable CTA instead
/// of the generic "Download failed" the post-spawn dep_ch failure
/// otherwise produces.
pub fn ensure_ffmpeg_in_path(
    composed_path: &OsStr,
    is_executable: impl Fn(&Path) -> bool,
) -> Result<()> {
    let _ = composed_path;
    let _ = is_executable;
    todo!("test(red): real impl lands in the paired feat(green) commit")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &OsStr) -> Vec<PathBuf> {
        std::env::split_paths(s).collect()
    }

    fn join(parts: &[&str]) -> OsString {
        let pbs: Vec<PathBuf> = parts.iter().map(PathBuf::from).collect();
        std::env::join_paths(&pbs).expect("join_paths in test fixture")
    }

    #[test]
    fn bundled_bin_is_prepended_to_inherited_path() {
        let bundled = PathBuf::from("/bundle/bin");
        let inherited = join(&["/usr/bin", "/bin"]);
        let got = compose_anicli_path(Some(&bundled), None, Some(&inherited));
        let parts = split(&got);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
        assert_eq!(parts[1], PathBuf::from("/usr/bin"));
        assert_eq!(parts[2], PathBuf::from("/bin"));
    }

    #[test]
    fn no_bundled_bin_returns_inherited_unchanged() {
        let inherited = join(&["/usr/bin", "/bin"]);
        let got = compose_anicli_path(None, None, Some(&inherited));
        assert_eq!(split(&got), split(&inherited));
    }

    #[test]
    fn path_override_takes_precedence_over_inherited() {
        let inherited = join(&["/usr/bin", "/bin"]);
        let got = compose_anicli_path(None, Some("/shim:/other"), Some(&inherited));
        let parts = split(&got);
        // Override wins; the inherited /usr/bin path is dropped entirely.
        // We don't assert exact equality with the override string because
        // join_paths re-canonicalises the separator per host platform —
        // instead split the override the same way and compare lists.
        let expected: Vec<PathBuf> = std::env::split_paths(OsStr::new("/shim:/other")).collect();
        assert_eq!(parts, expected);
    }

    #[test]
    fn bundled_prepends_path_override_too() {
        let bundled = PathBuf::from("/bundle/bin");
        let got = compose_anicli_path(Some(&bundled), Some("/shim"), None);
        let parts = split(&got);
        assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
        assert_eq!(parts[1], PathBuf::from("/shim"));
    }

    #[test]
    fn no_bundled_no_inherited_falls_back_to_default() {
        let got = compose_anicli_path(None, None, None);
        let parts = split(&got);
        let expected: Vec<PathBuf> = std::env::split_paths(OsStr::new(FALLBACK_PATH)).collect();
        assert_eq!(parts, expected);
    }

    #[test]
    fn ensure_ffmpeg_returns_ok_when_executable_in_first_dir() {
        let path = std::env::join_paths(["/bundle/bin", "/usr/bin"].map(PathBuf::from)).unwrap();
        let r = ensure_ffmpeg_in_path(&path, |p| {
            p == Path::new("/bundle/bin/ffmpeg") || p == Path::new("/bundle/bin/ffmpeg.exe")
        });
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn ensure_ffmpeg_returns_ok_when_executable_in_later_dir() {
        let path =
            std::env::join_paths(["/no/ffmpeg/here", "/usr/bin"].map(PathBuf::from)).unwrap();
        let r = ensure_ffmpeg_in_path(&path, |p| {
            p == Path::new("/usr/bin/ffmpeg") || p == Path::new("/usr/bin/ffmpeg.exe")
        });
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn ensure_ffmpeg_returns_ffmpeg_missing_when_absent_everywhere() {
        let path = std::env::join_paths(["/a", "/b", "/c"].map(PathBuf::from)).unwrap();
        let r = ensure_ffmpeg_in_path(&path, |_| false);
        assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
    }

    #[test]
    fn ensure_ffmpeg_returns_ffmpeg_missing_for_empty_path() {
        // join_paths can't produce an empty value on every platform
        // (Windows allows it, Unix doesn't), so build directly.
        let path = OsString::new();
        let r = ensure_ffmpeg_in_path(&path, |_| true);
        assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
    }

    #[test]
    fn bundled_alone_emits_just_the_bundled_dir() {
        let bundled = PathBuf::from("/bundle/bin");
        let got = compose_anicli_path(Some(&bundled), None, None);
        let parts = split(&got);
        // Bundled first, then the FALLBACK_PATH components.
        assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
        let fallback: Vec<PathBuf> = std::env::split_paths(OsStr::new(FALLBACK_PATH)).collect();
        assert_eq!(&parts[1..], fallback.as_slice());
    }
}
