//! Syncplay-specific config helpers.
//!
//! Extracted from `config/mod.rs` so the per-OS default + its 3
//! round-trip tests don't tip that file's aggregate ccn over 30 (the
//! firm `crap.high_risk_le` ceiling). The function is `pub(super)`
//! because `Config::syncplay_binary`'s serde default sits inside
//! `config/mod.rs` and needs to name it directly.

/// Per-OS default Syncplay binary path.
///
/// - Linux: `"syncplay"`. The .deb / AppImage install drops the
///   binary onto PATH; fresh installs that put it elsewhere can
///   Browse… in settings.
/// - Windows: `"C:\\Program Files\\Syncplay\\Syncplay.exe"`. The
///   official NSIS installer doesn't touch %PATH%, so falling back
///   to bare `"syncplay"` would fail spawn for the vast majority of
///   Windows users with a default install. The vendored ani-cli
///   script handles this the same way (lines 504-509) by hard-
///   coding `C:\Program Files (x86)\Syncplay\`; the 64-bit
///   installer landed on the non-(x86) path years ago, so we
///   default to that. Users on the legacy 32-bit installer or a
///   portable extract Browse… to point at their binary.
/// - macOS: the Syncplay.app inner executable since macOS GUI .app
///   bundles don't drop their executable into PATH.
///
/// Used by the `Config::syncplay_binary` serde default so old
/// configs decode with a sensible per-platform value.
#[must_use]
pub(super) fn default_syncplay_binary() -> String {
    #[cfg(target_os = "macos")]
    {
        "/Applications/Syncplay.app/Contents/MacOS/syncplay".into()
    }
    #[cfg(target_os = "windows")]
    {
        "C:\\Program Files\\Syncplay\\Syncplay.exe".into()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "syncplay".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syncplay_binary_default_per_os() {
        // Per-OS defaults. macOS + Windows need explicit paths
        // because their standard installers don't drop the binary
        // into PATH (Syncplay.app's inner executable on macOS;
        // C:\Program Files\Syncplay\Syncplay.exe on Windows). Linux
        // uses bare `"syncplay"` since the .deb / AppImage install
        // does land on PATH.
        let got = default_syncplay_binary();
        #[cfg(target_os = "macos")]
        assert_eq!(got, "/Applications/Syncplay.app/Contents/MacOS/syncplay");
        #[cfg(target_os = "windows")]
        assert_eq!(got, "C:\\Program Files\\Syncplay\\Syncplay.exe");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert_eq!(got, "syncplay");
    }
}
