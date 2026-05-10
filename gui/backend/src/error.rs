//! `AniError` — the single error type that crosses every Tauri command
//! boundary.
//!
//! Every variant maps to a stable i18n key returned to the frontend.
//! Localized strings are resolved by the frontend (Paraglide), never by the
//! backend. See [`crate::i18n`] for the canonical key list.

use serde::Serialize;
use thiserror::Error;

/// Result alias for backend operations.
pub type Result<T, E = AniError> = std::result::Result<T, E>;

/// Any failure that may occur in the backend. Variants serialize to the
/// frontend with a `kind` discriminator and an i18n `key` so the UI can
/// localize without parsing the message.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AniError {
    /// The vendored `ani-cli` script reported an internal failure.
    #[error("scraper error")]
    Scraper {
        /// i18n key under `error.scraper.*`.
        key: &'static str,
    },

    /// The `ani-cli` subprocess didn't finish within its timeout.
    #[error("scraper timed out")]
    Timeout,

    /// Search returned zero results.
    #[error("no results")]
    NoResults,

    /// Stdout from `ani-cli` did not match the expected debug-mode shape.
    #[error("parse failed: {detail}")]
    ParseFailed {
        /// Free-text detail for logs only — not surfaced to the user.
        detail: String,
    },

    /// `ani-cli` was not found on PATH or under the bundled resource dir.
    #[error("missing ani-cli binary")]
    MissingBinary,

    /// Windows-readiness: no `bash.exe` reachable. The locator probes
    /// PATH, Git for Windows install dirs, and the scoop default; if
    /// none point at an executable, this fires. The frontend renders
    /// a one-link install pointer for Git for Windows.
    #[error("missing bash.exe (install Git for Windows)")]
    BashMissing,

    /// An upstream HTTP request returned a non-success status.
    #[error("upstream {status}")]
    Upstream {
        /// HTTP status code from the upstream server.
        status: u16,
    },

    /// A network-layer failure (connection refused, DNS, TLS).
    #[error("network error")]
    Network,

    /// External-player binary couldn't be spawned (not on PATH, or
    /// the configured path doesn't point at an executable). The
    /// `binary` field carries the configured player name so the UI
    /// can name the failed command in the error toast.
    #[error("player spawn failed: {binary}")]
    PlayerSpawnFailed {
        /// The player command the user configured (e.g. `"vlc"`,
        /// `"/usr/bin/mpv"`). Surfaced verbatim in the localized
        /// error message via the `{binary}` placeholder.
        binary: String,
    },

    /// Cache (SQLite) operation failed.
    #[error("cache error")]
    Cache,

    /// Filesystem I/O failure.
    #[error("io error")]
    Io,

    /// Configuration file (TOML) parse or write failure.
    #[error("config error")]
    Config,

    /// Metadata source (Kitsu or AniList) returned malformed data.
    #[error("metadata source")]
    Metadata,

    /// Stream session token was missing, expired, or signature-invalid.
    #[error("invalid stream token")]
    InvalidToken,
}

impl AniError {
    /// Stable i18n key used by the frontend to look up a localized message.
    /// Variants without their own key fall back to a top-level key by
    /// variant name.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Scraper { key } => key,
            Self::Timeout => "error.scraper.timeout",
            Self::NoResults => "error.search.no_results",
            Self::ParseFailed { .. } => "error.scraper.parse_failed",
            Self::MissingBinary => "error.scraper.missing_binary",
            Self::BashMissing => "error.bash.missing",
            Self::PlayerSpawnFailed { .. } => "error.player.spawn_failed",
            Self::Upstream { .. } => "error.network.upstream",
            Self::Network => "error.network.unreachable",
            Self::Cache => "error.cache.generic",
            Self::Io => "error.io.generic",
            Self::Config => "error.config.parse",
            Self::Metadata => "error.metadata.source",
            Self::InvalidToken => "error.stream.invalid_token",
        }
    }
}

impl From<reqwest::Error> for AniError {
    fn from(_: reqwest::Error) -> Self {
        AniError::Network
    }
}

impl From<rusqlite::Error> for AniError {
    fn from(_: rusqlite::Error) -> Self {
        AniError::Cache
    }
}

impl From<std::io::Error> for AniError {
    fn from(_: std::io::Error) -> Self {
        AniError::Io
    }
}

impl From<serde_json::Error> for AniError {
    fn from(e: serde_json::Error) -> Self {
        AniError::ParseFailed {
            detail: e.to_string(),
        }
    }
}

impl From<toml::de::Error> for AniError {
    fn from(_: toml::de::Error) -> Self {
        AniError::Config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_stable_key() {
        // A representative of each variant — if a new variant lands without
        // a matching arm in `key()`, this test forces the author to think
        // about its i18n key.
        let cases = [
            AniError::Scraper {
                key: "error.scraper.custom_test_key",
            },
            AniError::Timeout,
            AniError::NoResults,
            AniError::ParseFailed { detail: "x".into() },
            AniError::MissingBinary,
            AniError::BashMissing,
            AniError::PlayerSpawnFailed {
                binary: "vlc".into(),
            },
            AniError::Upstream { status: 503 },
            AniError::Network,
            AniError::Cache,
            AniError::Io,
            AniError::Config,
            AniError::Metadata,
            AniError::InvalidToken,
        ];
        for c in cases {
            let k = c.key();
            assert!(
                k.starts_with("error."),
                "every error key starts with 'error.': got {k:?} for {c:?}"
            );
            assert!(!k.is_empty());
        }
    }

    #[test]
    fn serializes_with_kind_discriminator() {
        let err = AniError::NoResults;
        let s = serde_json::to_string(&err).expect("serializes");
        assert!(s.contains("\"kind\""), "serialized form has kind tag: {s}");
        assert!(s.contains("no_results"), "snake_case discriminant: {s}");
    }

    #[test]
    fn bash_missing_serializes_with_a_dedicated_kind_and_key() {
        // Windows-readiness: the GUI needs bash.exe (Git for Windows)
        // to drive the POSIX ani-cli script. When the locator returns
        // None at startup, the frontend renders an install-Git-for-
        // Windows pointer; that branch keys off this dedicated
        // variant rather than collapsing into MissingBinary (which is
        // the ani-cli-not-found error and would mislead the message).
        let err = AniError::BashMissing;
        let s = serde_json::to_string(&err).expect("serializes");
        assert!(
            s.contains("\"kind\":\"bash_missing\""),
            "snake_case kind: {s}"
        );
        assert_eq!(err.key(), "error.bash.missing");
    }

    #[test]
    fn player_spawn_failed_carries_the_configured_binary_name() {
        // The frontend toast should be able to name *which* player
        // failed — generic "missing binary" wasn't actionable. Pin
        // that the JSON the frontend receives includes the binary.
        let err = AniError::PlayerSpawnFailed {
            binary: "vlc".into(),
        };
        let s = serde_json::to_string(&err).expect("serializes");
        assert!(
            s.contains("\"binary\":\"vlc\""),
            "serialized form has binary field: {s}"
        );
        assert!(
            s.contains("\"kind\":\"player_spawn_failed\""),
            "serialized form has snake_case kind: {s}"
        );
        assert_eq!(err.key(), "error.player.spawn_failed");
    }

    /// Display impl drives `tracing::error!("{err}")` lines and the
    /// fallback message text in tests. Pin a representative subset
    /// so a stray `#[error("…")]` rewrite gets caught.
    #[test]
    fn display_messages_match_thiserror_attributes() {
        assert_eq!(format!("{}", AniError::Timeout), "scraper timed out");
        assert_eq!(format!("{}", AniError::NoResults), "no results");
        assert_eq!(format!("{}", AniError::Network), "network error");
        assert_eq!(
            format!("{}", AniError::Upstream { status: 503 }),
            "upstream 503"
        );
        assert_eq!(
            format!(
                "{}",
                AniError::ParseFailed {
                    detail: "stdout shape".into()
                }
            ),
            "parse failed: stdout shape"
        );
    }

    /// Each From impl collapses an upstream error into a single
    /// `AniError` variant — the frontend never sees the underlying
    /// reqwest / rusqlite / serde / toml type. A bug in one of these
    /// would surface as the wrong i18n key for the user.
    #[test]
    fn rusqlite_error_maps_to_cache_variant() {
        let sqlite_err = rusqlite::Error::ExecuteReturnedResults;
        let mapped: AniError = sqlite_err.into();
        assert!(matches!(mapped, AniError::Cache));
        assert_eq!(mapped.key(), "error.cache.generic");
    }

    #[test]
    fn io_error_maps_to_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let mapped: AniError = io_err.into();
        assert!(matches!(mapped, AniError::Io));
        assert_eq!(mapped.key(), "error.io.generic");
    }

    #[test]
    fn serde_error_carries_its_detail_into_parse_failed() {
        // The detail field is for logs, not user-facing copy. Pin
        // that the conversion preserves it so debugging stays sane.
        let serde_err = serde_json::from_str::<u32>("not a number").unwrap_err();
        let mapped: AniError = serde_err.into();
        match mapped {
            AniError::ParseFailed { detail } => assert!(!detail.is_empty()),
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }

    #[test]
    fn toml_error_maps_to_config_variant() {
        let toml_err: toml::de::Error =
            toml::from_str::<toml::Value>("not = valid = toml").unwrap_err();
        let mapped: AniError = toml_err.into();
        assert!(matches!(mapped, AniError::Config));
        assert_eq!(mapped.key(), "error.config.parse");
    }
}
