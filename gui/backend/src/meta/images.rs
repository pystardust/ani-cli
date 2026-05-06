//! On-disk image byte cache.
//!
//! Files live at `<cache_dir>/<hash[0..2]>/<hash>.<ext>`. The hash is the
//! first 16 bytes (32 hex chars) of SHA-256 over the source URL — enough
//! to dedupe Kitsu/AniList URLs without realistic collision risk.
//!
//! The filesystem itself is the cache index for the hot read path; the
//! `image_index` SQLite table (M2.2) is only updated for diagnostics and
//! future LRU/TTL bookkeeping. A cache hit avoids any DB call.
//!
//! All writes are atomic (`tmp` file + `rename`) so partially-downloaded
//! bytes never become a cached entry.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{AniError, Result};

/// Stable 32-hex-char hash for a URL. First 16 bytes of SHA-256.
#[must_use]
pub fn hash_url(url: &str) -> String {
    let mut h = Sha256::new();
    h.update(url.as_bytes());
    let bytes = h.finalize();
    bytes.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Derive the on-disk path for a given hash + extension. The first two
/// hex chars are used as a bucket directory.
#[must_use]
pub fn disk_path(cache_dir: &Path, hash: &str, ext: &str) -> PathBuf {
    cache_dir.join(&hash[..2]).join(format!("{hash}.{ext}"))
}

/// Sniff content-type + extension from a URL's path. Returns the pair
/// `(mime_type, extension_no_dot)`. Falls back to `("application/octet-stream", "bin")`
/// when no recognized image extension is found.
#[must_use]
pub fn sniff_extension(url: &str) -> (&'static str, &'static str) {
    let lower = url.to_lowercase();
    if lower.contains(".jpg") || lower.contains(".jpeg") {
        ("image/jpeg", "jpg")
    } else if lower.contains(".png") {
        ("image/png", "png")
    } else if lower.contains(".webp") {
        ("image/webp", "webp")
    } else if lower.contains(".gif") {
        ("image/gif", "gif")
    } else if lower.contains(".svg") {
        ("image/svg+xml", "svg")
    } else {
        ("application/octet-stream", "bin")
    }
}

/// Read cached bytes if the file is on disk.
#[must_use]
pub fn read_cached(cache_dir: &Path, url: &str) -> Option<(Vec<u8>, &'static str)> {
    let hash = hash_url(url);
    let (mime, ext) = sniff_extension(url);
    let path = disk_path(cache_dir, &hash, ext);
    std::fs::read(&path).ok().map(|bytes| (bytes, mime))
}

/// Fetch the URL via reqwest, write atomically to the cache, return
/// bytes + mime. Creates the bucket directory if absent.
///
/// # Errors
/// - [`AniError::Upstream`] for non-2xx responses.
/// - [`AniError::Network`] for transport failure.
/// - [`AniError::Io`] for filesystem failure.
pub async fn fetch_and_store(
    client: &reqwest::Client,
    cache_dir: &Path,
    url: &str,
) -> Result<(Vec<u8>, &'static str)> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    if !resp.status().is_success() {
        return Err(AniError::Upstream {
            status: resp.status().as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?.to_vec();

    let hash = hash_url(url);
    let (mime, ext) = sniff_extension(url);
    let final_path = disk_path(cache_dir, &hash, ext);
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
    }
    let tmp_path = final_path.with_extension(format!("{ext}.tmp"));
    std::fs::write(&tmp_path, &bytes).map_err(|_| AniError::Io)?;
    std::fs::rename(&tmp_path, &final_path).map_err(|_| AniError::Io)?;

    Ok((bytes, mime))
}

/// Cache hit → serve from disk. Cache miss → fetch + store + serve.
///
/// # Errors
/// Inherits from [`fetch_and_store`].
pub async fn get_or_fetch(
    client: &reqwest::Client,
    cache_dir: &Path,
    url: &str,
) -> Result<(Vec<u8>, &'static str)> {
    if let Some(cached) = read_cached(cache_dir, url) {
        return Ok(cached);
    }
    fetch_and_store(client, cache_dir, url).await
}

/// Resolve an `image://` request to bytes + mime, going through the cache
/// layer. Used by the Tauri custom-protocol handler in `lib::run`.
///
/// `request_uri` is the URL the webview asked for, in `image://host/path`
/// shape.
///
/// # Errors
/// - [`AniError::ParseFailed`] when the URI isn't a valid `image://` URL.
/// - [`AniError::Upstream`], [`AniError::Network`], [`AniError::Io`] from
///   the underlying fetch + store path.
pub async fn handle_protocol_request(
    client: &reqwest::Client,
    cache_dir: &Path,
    request_uri: &str,
) -> Result<(Vec<u8>, &'static str)> {
    let upstream = upstream_from_protocol_uri(request_uri)?;
    get_or_fetch(client, cache_dir, &upstream).await
}

/// Reconstruct the upstream HTTPS URL from an `image://` protocol URI.
///
/// `image://media.kitsu.app/anime/12/poster.jpg` becomes
/// `https://media.kitsu.app/anime/12/poster.jpg`.
///
/// Tauri requires the protocol scheme to use a host segment, and webkit2gtk
/// normalizes the URL into `image://<host>/<path>` form. Reconstruction is
/// therefore a literal scheme swap.
///
/// # Errors
/// [`AniError::ParseFailed`] when the input doesn't have an `image://`
/// scheme or is missing a host.
pub fn upstream_from_protocol_uri(uri: &str) -> Result<String> {
    let parsed = url::Url::parse(uri).map_err(|e| AniError::ParseFailed {
        detail: format!("image:// uri parse: {e}"),
    })?;
    if parsed.scheme() != "image" {
        return Err(AniError::ParseFailed {
            detail: format!("expected image:// scheme, got {}://", parsed.scheme()),
        });
    }
    let Some(host) = parsed.host_str() else {
        return Err(AniError::ParseFailed {
            detail: "image:// uri missing host".into(),
        });
    };
    let path = parsed.path();
    let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
    Ok(format!("https://{host}{path}{query}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hash_url_is_deterministic_and_32_hex() {
        let h = hash_url("https://media.kitsu.app/anime/12/poster.jpg");
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Same URL → same hash.
        assert_eq!(h, hash_url("https://media.kitsu.app/anime/12/poster.jpg"));
        // Different URL → different hash.
        assert_ne!(h, hash_url("https://media.kitsu.app/anime/13/poster.jpg"));
    }

    #[test]
    fn disk_path_buckets_by_first_two_hex_chars() {
        let p = disk_path(
            Path::new("/cache"),
            "abcd1234ef567890aabbccddeeff0011",
            "jpg",
        );
        assert_eq!(
            p,
            Path::new("/cache/ab/abcd1234ef567890aabbccddeeff0011.jpg")
        );
    }

    #[test]
    fn sniff_extension_recognizes_common_image_types() {
        assert_eq!(sniff_extension("https://x/y.jpg"), ("image/jpeg", "jpg"));
        assert_eq!(sniff_extension("https://x/y.JPEG"), ("image/jpeg", "jpg"));
        assert_eq!(sniff_extension("https://x/y.png"), ("image/png", "png"));
        assert_eq!(sniff_extension("https://x/y.webp"), ("image/webp", "webp"));
        assert_eq!(sniff_extension("https://x/y.gif"), ("image/gif", "gif"));
        assert_eq!(sniff_extension("https://x/y.svg"), ("image/svg+xml", "svg"));
        assert_eq!(
            sniff_extension("https://x/no-extension"),
            ("application/octet-stream", "bin")
        );
    }

    #[test]
    fn read_cached_returns_none_for_missing_file() {
        let dir = TempDir::new().unwrap();
        assert!(read_cached(dir.path(), "https://x/y.jpg").is_none());
    }

    #[test]
    fn read_cached_returns_bytes_for_existing_file() {
        let dir = TempDir::new().unwrap();
        let url = "https://x/y.jpg";
        let hash = hash_url(url);
        let path = disk_path(dir.path(), &hash, "jpg");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"hello").unwrap();
        let (bytes, mime) = read_cached(dir.path(), url).expect("cache hit");
        assert_eq!(bytes, b"hello");
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn upstream_from_protocol_uri_swaps_scheme_to_https() {
        assert_eq!(
            upstream_from_protocol_uri("image://media.kitsu.app/anime/12/poster.jpg").unwrap(),
            "https://media.kitsu.app/anime/12/poster.jpg"
        );
    }

    #[test]
    fn upstream_from_protocol_uri_preserves_query_string() {
        assert_eq!(
            upstream_from_protocol_uri("image://media.kitsu.app/x.jpg?v=2").unwrap(),
            "https://media.kitsu.app/x.jpg?v=2"
        );
    }

    #[test]
    fn upstream_from_protocol_uri_rejects_wrong_scheme() {
        assert!(matches!(
            upstream_from_protocol_uri("https://x/y.jpg"),
            Err(AniError::ParseFailed { .. })
        ));
    }

    #[test]
    fn upstream_from_protocol_uri_rejects_missing_host() {
        assert!(matches!(
            upstream_from_protocol_uri("image:///just-a-path"),
            Err(AniError::ParseFailed { .. })
        ));
    }

    // — Properties ────────────────────────────────────────────────────
    //
    // The on-disk image cache is filesystem-keyed by `hash_url(src)`,
    // so two invariants must hold for the cache to be a cache at all:
    //   1. Determinism: same URL always hashes to the same string.
    //   2. Shape: the hash is exactly 32 lowercase hex chars, used as
    //      the filename + first-two-chars bucket directory.
    //
    // Collision-resistance is delegated to SHA-256 itself; we don't
    // assert it (would require billions of cases to exercise). What
    // we DO check is that distinct inputs in our actual use space
    // (Kitsu/AniList CDN URLs) produce distinct hashes — small but
    // catches accidental truncation that destroys the property.
    use proptest::prelude::*;

    proptest! {
        /// Same URL → same hash, every call.
        #[test]
        fn hash_url_is_deterministic_property(
            url in r"https://[a-zA-Z0-9./_-]{5,200}",
        ) {
            prop_assert_eq!(hash_url(&url), hash_url(&url));
        }

        /// Hash is always exactly 32 lowercase hex characters,
        /// regardless of input length or character mix.
        #[test]
        fn hash_url_is_32_lowercase_hex(
            url in r"[\PC]{1,500}",
        ) {
            let h = hash_url(&url);
            prop_assert_eq!(h.len(), 32);
            prop_assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }

        /// Different URLs in our actual input space produce different
        /// hashes. This is a weaker statement than collision-resistance
        /// but rules out the "bug truncates input" failure mode.
        #[test]
        fn distinct_urls_hash_distinctly(
            a in r"https://[a-zA-Z0-9./_-]{5,80}",
            b in r"https://[a-zA-Z0-9./_-]{5,80}",
        ) {
            prop_assume!(a != b);
            prop_assert_ne!(hash_url(&a), hash_url(&b));
        }
    }
}
