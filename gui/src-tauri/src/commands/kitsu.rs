//! Kitsu IPC commands — search + detail, transparently cached through
//! the SQLite `meta_cache` table.
//!
//! On a hit the cached body is JSON-deserialized back into our typed
//! [`KitsuAnimeRef`] and returned without any HTTP. On a miss the live
//! Kitsu client is invoked, the response is serialized back into JSON
//! and stored under the same key, and only then returned.
//!
//! Cache TTLs come from [`crate::cache::ttl`]:
//! - Search results: `DISCOVERY_TTL` (6h) — popular anime is stable, but
//!   trending changes within a day; this is the right tradeoff between
//!   responsiveness and Kitsu API volume.
//! - Anime detail: `ANIME_DETAIL_TTL` (7d) — synopsis / titles / posters
//!   change rarely.

use crate::app::AppState;
use crate::cache::ttl::{ANIME_DETAIL_TTL, DISCOVERY_TTL, TRENDING_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuEpisode};

/// Max search result page size we ask Kitsu for. Keeping this conservative
/// because the UI only renders a handful of cards before scrolling needs
/// pagination (which we don't support yet).
const SEARCH_PAGE_LIMIT: u8 = 20;

/// Search Kitsu for anime by free-text. Cache key:
/// `kitsu:search:<normalized-query>`. Returns at most
/// [`SEARCH_PAGE_LIMIT`] hits.
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::search`] on cache miss.
pub async fn kitsu_search(state: &AppState, query: &str) -> Result<Vec<KitsuAnimeRef>> {
    let key = format!("kitsu:search:{}", normalize_query(query));
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(hits) = serde_json::from_str::<Vec<KitsuAnimeRef>>(&body) {
            return Ok(hits);
        }
        // Cached body deserialized as something else — treat as miss and
        // rebuild rather than hand the frontend bad data.
    }
    let hits = state.kitsu.search(query, SEARCH_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&hits) {
        // TTL conversion can't fail in practice; clamp via try_from.
        let _ = meta_cache_put(&state.cache_pool, &key, &body, DISCOVERY_TTL.as_secs());
    }
    Ok(hits)
}

/// Currently-airing anime ranked by user count — a usable proxy for
/// "trending" until the AniList client lands. Cache key: `kitsu:trending`,
/// TTL [`TRENDING_TTL`] (1 hour).
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::currently_airing_by_user_count`]
/// on cache miss.
pub async fn kitsu_trending(state: &AppState) -> Result<Vec<KitsuAnimeRef>> {
    discovery_cached(
        state,
        "kitsu:trending",
        TRENDING_TTL.as_secs(),
        |limit| async move { state.kitsu.currently_airing_by_user_count(limit).await },
    )
    .await
}

/// Top-rated anime (averageRating ≥ 70/100). Cache key:
/// `kitsu:top_rated`, TTL [`DISCOVERY_TTL`] (6 hours).
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::top_rated`] on miss.
pub async fn kitsu_top_rated(state: &AppState) -> Result<Vec<KitsuAnimeRef>> {
    discovery_cached(
        state,
        "kitsu:top_rated",
        DISCOVERY_TTL.as_secs(),
        |limit| async move { state.kitsu.top_rated(limit).await },
    )
    .await
}

/// Page size used by the discovery rows. Hard-coded so the cache key is
/// stable; if we ever expose `limit` as an arg, the cache key has to
/// include it.
const DISCOVERY_PAGE_LIMIT: u8 = 20;

/// Generic cached-fetch helper for the discovery rows.
///
/// On a hit, the cached body is JSON-deserialized back into the typed list.
/// On a miss, the supplied async fetcher runs, the result is cached, and
/// returned. Errors aren't cached (next call retries).
async fn discovery_cached<F, Fut>(
    state: &AppState,
    cache_key: &str,
    ttl_seconds: u64,
    fetch: F,
) -> Result<Vec<KitsuAnimeRef>>
where
    F: FnOnce(u8) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<KitsuAnimeRef>>>,
{
    if let Some(body) = meta_cache_get(&state.cache_pool, cache_key)? {
        if let Ok(hits) = serde_json::from_str::<Vec<KitsuAnimeRef>>(&body) {
            return Ok(hits);
        }
    }
    let hits = fetch(DISCOVERY_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&hits) {
        let _ = meta_cache_put(&state.cache_pool, cache_key, &body, ttl_seconds);
    }
    Ok(hits)
}

/// First N episodes for a Kitsu anime. Cache key: `kitsu:episodes:<id>`,
/// TTL [`ANIME_DETAIL_TTL`] (7 days). Note that for currently-airing
/// shows new episodes appear weekly; the 7-day TTL means a busy fan can
/// see a stale list for up to a week, which is the right tradeoff for a
/// catalog browse — better than hammering Kitsu on every detail-page open.
const EPISODES_PAGE_LIMIT: u8 = 24;

/// Fetch episodes for an anime, capped at [`EPISODES_PAGE_LIMIT`].
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::episodes`] on miss.
pub async fn kitsu_episodes(state: &AppState, anime_id: &str) -> Result<Vec<KitsuEpisode>> {
    let key = format!("kitsu:episodes:{anime_id}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(eps) = serde_json::from_str::<Vec<KitsuEpisode>>(&body) {
            return Ok(eps);
        }
    }
    let eps = state.kitsu.episodes(anime_id, EPISODES_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&eps) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
    }
    Ok(eps)
}

/// Fetch a single anime by Kitsu id. Cache key: `kitsu:anime:<id>`.
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::anime_detail`] on miss.
pub async fn kitsu_anime_detail(state: &AppState, id: &str) -> Result<KitsuAnimeRef> {
    let key = format!("kitsu:anime:{id}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(detail) = serde_json::from_str::<KitsuAnimeRef>(&body) {
            return Ok(detail);
        }
    }
    let detail = state.kitsu.anime_detail(id).await?;
    if let Ok(body) = serde_json::to_string(&detail) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
    }
    Ok(detail)
}

/// Lowercase + collapse internal whitespace + trim. Stable cache key
/// regardless of how the user types the query.
fn normalize_query(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, SCRAPER_CONCURRENCY};
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SEARCH_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/search_one_piece.json");
    const DETAIL_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

    fn state_with_kitsu_at(uri: &str) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: PathBuf::from("/x"),
            history_path: PathBuf::from("/y/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::with_base(reqwest::Client::new(), uri),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        }
    }

    #[test]
    fn normalize_query_collapses_whitespace_and_lowercases() {
        assert_eq!(normalize_query("One Piece"), "one piece");
        assert_eq!(normalize_query("  ONE   PIECE  "), "one piece");
        assert_eq!(normalize_query("\tone\npiece"), "one piece");
    }

    #[tokio::test]
    async fn kitsu_search_caches_after_first_call() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[text]", "one piece"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1) // <-- must only hit upstream once
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_search(&state, "one piece").await.expect("ok");
        let second = kitsu_search(&state, "one piece").await.expect("ok");
        assert_eq!(first.len(), 5);
        assert_eq!(first, second, "cache hit returns identical body");
    }

    #[tokio::test]
    async fn kitsu_search_normalizes_query_for_cache_key() {
        // Two queries that normalize to the same key should share the
        // same cached entry — upstream sees one request total.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let _ = kitsu_search(&state, "  One   Piece  ").await.unwrap();
        let _ = kitsu_search(&state, "ONE PIECE").await.unwrap();
        // mock.expect(1) does the assertion when the harness drops.
    }

    #[tokio::test]
    async fn kitsu_anime_detail_caches_after_first_call() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/12"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(DETAIL_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_anime_detail(&state, "12").await.unwrap();
        let second = kitsu_anime_detail(&state, "12").await.unwrap();
        assert_eq!(first.canonical_title, "One Piece");
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn kitsu_trending_caches_after_first_call_and_uses_filter_status_current() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[status]", "current"))
            .and(query_param("sort", "-userCount"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_trending(&state).await.expect("ok");
        let second = kitsu_trending(&state).await.expect("ok");
        assert_eq!(first.len(), 5);
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn kitsu_top_rated_caches_and_filters_by_minimum_average_rating() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[averageRating]", "70.."))
            .and(query_param("sort", "-averageRating"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let _ = kitsu_top_rated(&state).await.expect("ok");
        let _ = kitsu_top_rated(&state).await.expect("ok");
    }

    #[tokio::test]
    async fn kitsu_episodes_caches_after_first_call() {
        let mock = MockServer::start().await;
        const EPISODES_FIXTURE: &[u8] =
            include_bytes!("../../../../tests/fixtures/kitsu/episodes_one_piece.json");
        Mock::given(method("GET"))
            .and(path("/anime/12/episodes"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(EPISODES_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_episodes(&state, "12").await.expect("ok");
        let second = kitsu_episodes(&state, "12").await.expect("ok");
        assert_eq!(first.len(), 12);
        assert_eq!(first, second, "cache hit returns identical body");
    }

    #[tokio::test]
    async fn kitsu_search_propagates_upstream_errors_and_does_not_cache() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(ResponseTemplate::new(503))
            .expect(2) // <-- both calls hit upstream because errors are not cached
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        assert!(kitsu_search(&state, "anything").await.is_err());
        assert!(kitsu_search(&state, "anything").await.is_err());
    }
}
