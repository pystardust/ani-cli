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
///
/// AWS-signed S3 URLs (Kitsu's Backblaze posters for some sequels)
/// carry per-request `X-Amz-*` query params that change every time
/// Kitsu re-issues them. Hashing the full URL would miss the on-disk
/// cache for every refresh and force a re-fetch, which fails once
/// the 15-min signature window lapses. Strip those params first so
/// the same image keeps the same cache key across reissues.
#[must_use]
pub fn hash_url(url: &str) -> String {
    let canonical = canonicalize_for_cache(url);
    let mut h = Sha256::new();
    h.update(canonical.as_bytes());
    let bytes = h.finalize();
    bytes.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Drop AWS S3 signature query params (`X-Amz-Signature`,
/// `X-Amz-Date`, `X-Amz-Credential`, etc.) from a URL. Returns the
/// scheme + host + path verbatim and any non-`X-Amz-*` query params
/// in their original order. URLs without `X-Amz-` params pass
/// through unchanged.
fn canonicalize_for_cache(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|kv| !kv.to_ascii_lowercase().starts_with("x-amz-"))
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
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
/// On cache hit, bumps the file's mtime to "now" so LRU pruning
/// treats recent reads as fresh. Without this the prune would be
/// FIFO (oldest WRITTEN files first) rather than true LRU and
/// frequently-viewed posters would be evicted ahead of stale ones.
///
/// # Errors
/// Inherits from [`fetch_and_store`].
pub async fn get_or_fetch(
    client: &reqwest::Client,
    cache_dir: &Path,
    url: &str,
) -> Result<(Vec<u8>, &'static str)> {
    if let Some(cached) = read_cached(cache_dir, url) {
        // Bump mtime to "now" so LRU prune treats this hit as fresh.
        // std::fs::File::set_modified is in stable since 1.75 — we
        // pin 1.88, so it's available without an extra crate.
        let hash = hash_url(url);
        let (_, ext) = sniff_extension(url);
        let path = disk_path(cache_dir, &hash, ext);
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        return Ok(cached);
    }
    fetch_and_store(client, cache_dir, url).await
}

/// Total bytes used by every regular file under `cache_dir`. Walks
/// only the first-level bucket directories (the on-disk shape we
/// produce in [`disk_path`]); arbitrary nesting isn't expected and
/// would be skipped. Errors during the walk are non-fatal — files
/// that can't be statted just don't count toward the total.
#[must_use]
pub fn cache_size_bytes(cache_dir: &Path) -> u64 {
    let mut total: u64 = 0;
    let Ok(buckets) = std::fs::read_dir(cache_dir) else {
        return 0;
    };
    for bucket in buckets.flatten() {
        let Ok(files) = std::fs::read_dir(bucket.path()) else {
            continue;
        };
        for file in files.flatten() {
            if let Ok(meta) = file.metadata() {
                if meta.is_file() {
                    total = total.saturating_add(meta.len());
                }
            }
        }
    }
    total
}

/// Prune the on-disk cache down to `cap_bytes`. Walks every cached
/// file, sorts by modification time ascending (with [`get_or_fetch`]
/// touching mtime on hit, this serves as access time), deletes the
/// oldest files until the running total is at-or-below the cap.
///
/// No-op when the cache is already at-or-below the cap or when the
/// directory doesn't exist. Failures (a file disappeared mid-walk,
/// permissions error) are logged at warn level and don't propagate
/// — the prune is opportunistic.
pub fn prune_to_cap(cache_dir: &Path, cap_bytes: u64) {
    let mut entries: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = Vec::new();
    let Ok(buckets) = std::fs::read_dir(cache_dir) else {
        return;
    };
    for bucket in buckets.flatten() {
        let Ok(files) = std::fs::read_dir(bucket.path()) else {
            continue;
        };
        for file in files.flatten() {
            let Ok(meta) = file.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((file.path(), mtime, meta.len()));
        }
    }
    let total: u64 = entries.iter().map(|(_, _, n)| *n).sum();
    if total <= cap_bytes {
        return;
    }
    // Oldest mtime first → evict in that order.
    entries.sort_by_key(|(_, m, _)| *m);
    let mut running = total;
    for (path, _, size) in entries {
        if running <= cap_bytes {
            break;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => running = running.saturating_sub(size),
            Err(e) => tracing::warn!(path = ?path, error = ?e, "image-cache prune: remove failed"),
        }
    }
}

/// Fire-and-forget prune. Spawns a `spawn_blocking` task so the
/// directory walk doesn't stall an async caller. No-op when the
/// cache is already under cap.
pub fn schedule_prune(cache_dir: std::path::PathBuf, cap_bytes: u64) {
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || prune_to_cap(&cache_dir, cap_bytes)).await;
    });
}

/// Wipe the entire on-disk image cache. Used by the diagnostics
/// "Clear image cache" button. Walks bucket directories and removes
/// all regular files; bucket dirs themselves stay so the next write
/// doesn't have to recreate them. Errors are logged + propagated.
///
/// # Errors
/// [`AniError::Io`] when the cache_dir can't be read.
pub fn clear_all(cache_dir: &Path) -> Result<()> {
    let Ok(buckets) = std::fs::read_dir(cache_dir) else {
        // Missing dir means the cache is already empty; not an error.
        return Ok(());
    };
    for bucket in buckets.flatten() {
        let Ok(files) = std::fs::read_dir(bucket.path()) else {
            continue;
        };
        for file in files.flatten() {
            if file.metadata().is_ok_and(|m| m.is_file()) {
                if let Err(e) = std::fs::remove_file(file.path()) {
                    tracing::warn!(path = ?file.path(), error = ?e, "image-cache clear: remove failed");
                }
            }
        }
    }
    Ok(())
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
    fn hash_url_strips_aws_signature_params() {
        // Backblaze S3 signed URL — the signature changes per
        // request but the same image must keep one cache key.
        let base =
            "https://kitsu-production-media.s3.us-west-002.backblazeb2.com/anime/48069/poster.jpg";
        let signed_a = format!("{base}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=cred1&X-Amz-Date=20260508T000000Z&X-Amz-Expires=900&X-Amz-Signature=sig-a");
        let signed_b = format!("{base}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=cred1&X-Amz-Date=20260508T010000Z&X-Amz-Expires=900&X-Amz-Signature=sig-b");
        let plain = base.to_string();
        assert_eq!(hash_url(&signed_a), hash_url(&signed_b));
        assert_eq!(hash_url(&signed_a), hash_url(&plain));
    }

    #[test]
    fn hash_url_keeps_non_signature_query_params() {
        // Non-X-Amz query params still differentiate the cache key
        // (e.g. ?width=200 vs ?width=400 are conceptually different
        // images). `?` ordering is preserved.
        let a = hash_url("https://example.com/img.jpg?w=200");
        let b = hash_url("https://example.com/img.jpg?w=400");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_url_keeps_mixed_x_amz_and_normal_params_filters_only_aws() {
        // ?w=200&X-Amz-Signature=… should canonicalize to ?w=200.
        let mixed = "https://example.com/img.jpg?w=200&X-Amz-Signature=abc";
        let plain = "https://example.com/img.jpg?w=200";
        assert_eq!(hash_url(mixed), hash_url(plain));
    }

    #[test]
    fn cache_size_bytes_sums_files_under_buckets() {
        let td = tempfile::tempdir().expect("tempdir");
        let bucket = td.path().join("ab");
        std::fs::create_dir_all(&bucket).expect("mkdir");
        std::fs::write(bucket.join("ab1.jpg"), b"hello").expect("write");
        std::fs::write(bucket.join("ab2.jpg"), b"world!").expect("write");
        // Top-level non-dir entries are skipped (not a bucket).
        std::fs::write(td.path().join("loose"), b"ignore").expect("write");
        assert_eq!(cache_size_bytes(td.path()), 5 + 6);
    }

    #[test]
    fn cache_size_bytes_returns_zero_for_missing_dir() {
        assert_eq!(cache_size_bytes(std::path::Path::new("/nope/nada")), 0);
    }

    #[test]
    fn prune_to_cap_evicts_oldest_files_first() {
        let td = tempfile::tempdir().expect("tempdir");
        let bucket = td.path().join("aa");
        std::fs::create_dir_all(&bucket).expect("mkdir");
        let old = bucket.join("aa-old.jpg");
        let new = bucket.join("aa-new.jpg");
        std::fs::write(&old, vec![0u8; 1000]).expect("write");
        std::fs::write(&new, vec![0u8; 1000]).expect("write");
        // Backdate the "old" file by an hour so it sorts oldest.
        let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&old)
            .expect("open old");
        f.set_modified(an_hour_ago).expect("set mtime");
        // Cap at 1000 bytes — only one file fits. The old one should
        // get evicted, the new one stays.
        prune_to_cap(td.path(), 1000);
        assert!(!old.exists(), "old file should have been pruned");
        assert!(new.exists(), "new file should remain");
    }

    #[test]
    fn prune_to_cap_is_noop_when_under_cap() {
        let td = tempfile::tempdir().expect("tempdir");
        let bucket = td.path().join("aa");
        std::fs::create_dir_all(&bucket).expect("mkdir");
        let path = bucket.join("aa-small.jpg");
        std::fs::write(&path, b"tiny").expect("write");
        prune_to_cap(td.path(), 1_000_000);
        assert!(path.exists(), "file under cap shouldn't be pruned");
    }

    #[test]
    fn clear_all_removes_files_but_keeps_buckets() {
        let td = tempfile::tempdir().expect("tempdir");
        let bucket = td.path().join("aa");
        std::fs::create_dir_all(&bucket).expect("mkdir");
        let f = bucket.join("aa-x.jpg");
        std::fs::write(&f, b"x").expect("write");
        clear_all(td.path()).expect("clear");
        assert!(!f.exists(), "file should be gone");
        assert!(bucket.exists(), "bucket dir should remain (cheap reuse)");
    }

    #[test]
    fn clear_all_is_noop_for_missing_dir() {
        // Defensive — first run has no cache dir yet, clear shouldn't
        // fail just because there's nothing to remove.
        clear_all(std::path::Path::new("/no/such/path")).expect("ok on missing");
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
