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
use crate::cache::ttl::{
    ANIME_DETAIL_TTL, DISCOVERY_TTL, EPISODES_TTL, TITLE_MATCH_TTL, TRENDING_TTL,
};
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
///
/// Kitsu's episodes endpoint caps `page[limit]` at 20 (`code: 118 —
/// "Limit exceeds maximum page size of 20"` if you push higher), unlike
/// the anime search endpoint which allows 24. Hard-coded ceiling here.
const EPISODES_PAGE_LIMIT: u8 = 20;

/// Fetch a page of episodes for an anime. `page` is 1-based and
/// translates to a Kitsu `page[offset]` of `(page-1)*limit`. Each page
/// caches independently under `kitsu:episodes:<id>:p<n>`.
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::episodes`] on miss.
pub async fn kitsu_episodes(
    state: &AppState,
    anime_id: &str,
    page: u32,
) -> Result<Vec<KitsuEpisode>> {
    let p = page.max(1);
    let key = format!("kitsu:episodes:{anime_id}:p{p}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(eps) = serde_json::from_str::<Vec<KitsuEpisode>>(&body) {
            return Ok(eps);
        }
    }
    let eps = state
        .kitsu
        .episodes(anime_id, p, EPISODES_PAGE_LIMIT)
        .await?;
    if let Ok(body) = serde_json::to_string(&eps) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, EPISODES_TTL.as_secs());
    }
    Ok(eps)
}

/// Look up an anime by its slug — Kitsu's URL-stable identifier.
/// Used as a fallback when the text search doesn't include a sequel
/// (Kitsu's `filter[text]` ranks the most-popular sibling and drops
/// alternates entirely for some titles, e.g. JoJo Stone Ocean Part 2).
///
/// Cached under `kitsu:anime-slug:<slug>` with [`ANIME_DETAIL_TTL`].
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::anime_by_slug`].
pub async fn kitsu_anime_by_slug(state: &AppState, slug: &str) -> Result<Option<KitsuAnimeRef>> {
    let key = format!("kitsu:anime-slug:{slug}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(detail) = serde_json::from_str::<Option<KitsuAnimeRef>>(&body) {
            return Ok(detail);
        }
    }
    let detail = state.kitsu.anime_by_slug(slug).await?;
    if let Ok(body) = serde_json::to_string(&detail) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
    }
    Ok(detail)
}

/// Title-match cache: maps `(allmanga_title, cour) → kitsu_id`. Stored
/// in the shared `meta_cache` table under a `title-match:` key prefix
/// so the home page's Continue Watching strip skips a kitsuSearch +
/// pickKitsuMatch round-trip on subsequent loads.
///
/// Title is normalized (trim + lowercase) so cosmetic whitespace /
/// case differences hash to the same row. TTL is [`TITLE_MATCH_TTL`]
/// (30 days) — the title→id mapping rarely changes and re-resolving
/// is cheap when it does (just a stale-id detection on the detail
/// fetch, then re-search).
/// Cache key version. Bumped when the resolution rules change in a
/// way that previous mappings would now be wrong:
///
/// - v1: original picker (first hit). Wrote bad mappings for all
///   multi-cour entries since the picker collapsed siblings.
/// - v2: slug-fetch fallback for cour > 1 (commit 86e02d2). Old v1
///   mappings now orphaned, replaced by fresh v2 lookups.
const TITLE_MATCH_VERSION: u32 = 2;

fn title_match_key(title: &str, cour: u32) -> String {
    let normalized = title.trim().to_lowercase();
    format!("title-match:v{TITLE_MATCH_VERSION}:{normalized}:c{cour}")
}

/// Read the cached `(title, cour) → kitsu_id` mapping. Returns `None`
/// on miss; errors propagate the SQLite read failure.
pub fn title_match_get(state: &AppState, title: &str, cour: u32) -> Result<Option<String>> {
    meta_cache_get(&state.cache_pool, &title_match_key(title, cour))
}

/// Persist a `(title, cour) → kitsu_id` mapping under TITLE_MATCH_TTL.
/// Idempotent — re-puts overwrite the prior value, which is the
/// behaviour the picker wants when Kitsu re-catalogues an entry.
pub fn title_match_put(state: &AppState, title: &str, cour: u32, kitsu_id: &str) -> Result<()> {
    meta_cache_put(
        &state.cache_pool,
        &title_match_key(title, cour),
        kitsu_id,
        TITLE_MATCH_TTL.as_secs(),
    )
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
    async fn kitsu_episodes_caches_per_page_after_first_call() {
        let mock = MockServer::start().await;
        const EPISODES_FIXTURE: &[u8] =
            include_bytes!("../../../../tests/fixtures/kitsu/episodes_one_piece.json");
        Mock::given(method("GET"))
            .and(path("/anime/12/episodes"))
            .and(query_param("page[offset]", "0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(EPISODES_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_episodes(&state, "12", 1).await.expect("ok");
        let second = kitsu_episodes(&state, "12", 1).await.expect("ok");
        assert_eq!(first.len(), 12);
        assert_eq!(first, second, "cache hit returns identical body");
    }

    #[tokio::test]
    async fn kitsu_episodes_uses_offset_for_higher_pages() {
        let mock = MockServer::start().await;
        const EPISODES_FIXTURE: &[u8] =
            include_bytes!("../../../../tests/fixtures/kitsu/episodes_one_piece.json");
        Mock::given(method("GET"))
            .and(path("/anime/12/episodes"))
            .and(query_param("page[offset]", "20"))
            .and(query_param("page[limit]", "20"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(EPISODES_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let _ = kitsu_episodes(&state, "12", 2).await.expect("ok");
        // Asserted by mock.expect(1) — offset 20 hit exactly once.
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

    // — Slug lookup ——————————————————————————————————————————————————

    #[tokio::test]
    async fn kitsu_anime_by_slug_returns_some_on_match() {
        // The slug-lookup path exists because Kitsu's filter[text] drops
        // sequels for some titles (Stone Ocean Part 2 isn't in the text-
        // search response, but is reachable via filter[slug]). This test
        // pins the contract: hit /anime?filter[slug]=... → first hit.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[slug]", "stone-ocean-part-2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let got = kitsu_anime_by_slug(&state, "stone-ocean-part-2")
            .await
            .expect("ok");
        assert!(got.is_some(), "fixture has at least one hit");
    }

    #[tokio::test]
    async fn kitsu_anime_by_slug_caches_after_first_call() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[slug]", "demon-slayer"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(SEARCH_FIXTURE.to_vec()),
            )
            .expect(1) // <-- second call must hit the cache, not Kitsu
            .mount(&mock)
            .await;

        let state = state_with_kitsu_at(&mock.uri());
        let first = kitsu_anime_by_slug(&state, "demon-slayer")
            .await
            .expect("ok");
        let second = kitsu_anime_by_slug(&state, "demon-slayer")
            .await
            .expect("ok");
        assert_eq!(first.is_some(), second.is_some());
    }

    // — Title-match cache —————————————————————————————————————————————

    #[test]
    fn title_match_cache_round_trips_for_a_given_title_and_cour() {
        let state = state_with_kitsu_at("http://unused");
        title_match_put(&state, "Stone Ocean Part 2", 2, "kitsu-id-42").expect("put ok");
        let got = title_match_get(&state, "Stone Ocean Part 2", 2).expect("get ok");
        assert_eq!(got, Some("kitsu-id-42".to_string()));
    }

    #[test]
    fn title_match_cache_normalizes_whitespace_and_case() {
        // Normalization makes "  STONE OCEAN  PART 2  " hash to the
        // same row as "stone ocean  part 2". (Inner whitespace is left
        // alone — only trim + lowercase — but that's enough to soak
        // up the common variations from ani-cli's hsts.)
        let state = state_with_kitsu_at("http://unused");
        title_match_put(&state, "Stone Ocean", 1, "id-1").expect("put");
        let got_lc = title_match_get(&state, "stone ocean", 1).expect("get lowercased");
        let got_padded = title_match_get(&state, "  STONE OCEAN  ", 1).expect("get padded");
        assert_eq!(got_lc, Some("id-1".to_string()));
        assert_eq!(got_padded, Some("id-1".to_string()));
    }

    #[test]
    fn title_match_cache_separates_entries_by_cour() {
        // Cour is part of the key — Stone Ocean Part 1 and Part 2
        // must not collide on the cache row.
        let state = state_with_kitsu_at("http://unused");
        title_match_put(&state, "Stone Ocean", 1, "id-part1").expect("put p1");
        title_match_put(&state, "Stone Ocean", 2, "id-part2").expect("put p2");
        assert_eq!(
            title_match_get(&state, "Stone Ocean", 1).expect("get p1"),
            Some("id-part1".to_string())
        );
        assert_eq!(
            title_match_get(&state, "Stone Ocean", 2).expect("get p2"),
            Some("id-part2".to_string())
        );
    }

    #[test]
    fn title_match_cache_returns_none_for_unknown_titles() {
        let state = state_with_kitsu_at("http://unused");
        assert_eq!(
            title_match_get(&state, "Nothing here", 1).expect("get ok"),
            None
        );
    }

    #[test]
    fn title_match_cache_overwrites_on_re_put() {
        // When the picker resolves a different kitsu_id later (a
        // re-cataloguing on Kitsu, say), the latest put wins.
        let state = state_with_kitsu_at("http://unused");
        title_match_put(&state, "Demon Slayer", 1, "old").expect("put old");
        title_match_put(&state, "Demon Slayer", 1, "new").expect("put new");
        assert_eq!(
            title_match_get(&state, "Demon Slayer", 1).expect("get"),
            Some("new".to_string())
        );
    }
}
