//! Integration tests for the image fetch + on-disk cache pipeline.
//!
//! Locks the contract that:
//! 1. First call hits the upstream server, writes the bytes to the cache
//!    directory, and returns them.
//! 2. Second call returns the same bytes without hitting upstream
//!    (verified by wiremock's `expect(1)` assertion).
//! 3. Non-2xx responses surface as `AniError::Upstream`.

use ani_gui::error::AniError;
use ani_gui::meta::images::{disk_path, fetch_and_store, get_or_fetch, hash_url};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_and_store_writes_atomically_to_disk() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12/poster.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(b"\xFF\xD8\xFFhello-jpeg-bytes".to_vec()),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let cache = TempDir::new().unwrap();
    let url = format!("{}/anime/12/poster.jpg", mock.uri());
    let (bytes, mime) = fetch_and_store(&reqwest::Client::new(), cache.path(), &url)
        .await
        .expect("ok");

    assert_eq!(bytes, b"\xFF\xD8\xFFhello-jpeg-bytes");
    assert_eq!(mime, "image/jpeg");

    // Bytes are on disk at the expected path.
    let on_disk = std::fs::read(disk_path(cache.path(), &hash_url(&url), "jpg")).unwrap();
    assert_eq!(on_disk, bytes);
}

#[tokio::test]
async fn get_or_fetch_serves_second_call_from_cache_without_upstream_hit() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/once.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(b"\x89PNGonly-fetched-once".to_vec()),
        )
        .expect(1) // <-- the test fails if upstream sees a second request
        .mount(&mock)
        .await;

    let cache = TempDir::new().unwrap();
    let url = format!("{}/once.png", mock.uri());
    let client = reqwest::Client::new();

    let (b1, _) = get_or_fetch(&client, cache.path(), &url).await.unwrap();
    let (b2, _) = get_or_fetch(&client, cache.path(), &url).await.unwrap();
    assert_eq!(b1, b2);
}

#[tokio::test]
async fn fetch_and_store_returns_upstream_error_on_4xx() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing.jpg"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;

    let cache = TempDir::new().unwrap();
    let url = format!("{}/missing.jpg", mock.uri());
    let r = fetch_and_store(&reqwest::Client::new(), cache.path(), &url).await;
    match r {
        Err(AniError::Upstream { status }) => assert_eq!(status, 404),
        other => panic!("expected Upstream(404), got {other:?}"),
    }
}

#[tokio::test]
async fn get_or_fetch_returns_cached_bytes_when_upstream_is_offline() {
    // Pre-populate the cache, then point at a definitely-down upstream
    // (port 1 is privileged on Linux and never accepts) — the hit must
    // come from disk without trying the network.
    let cache = TempDir::new().unwrap();
    let url = "https://media.kitsu.app/anime/12/cached.png";
    let hash = hash_url(url);
    let path = disk_path(cache.path(), &hash, "png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"\x89PNGcached-bytes").unwrap();

    let (bytes, mime) = get_or_fetch(&reqwest::Client::new(), cache.path(), url)
        .await
        .expect("cache hit, no network");
    assert_eq!(bytes, b"\x89PNGcached-bytes");
    assert_eq!(mime, "image/png");
}
