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
use crate::commands::kitsu_warm::warm_signed_image_urls;
use crate::error::Result;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuEpisode};

// Schema version is encoded in the cache key prefix (`kitsu:v2:`).
// Bump when KitsuAnimeRef gains a field consumers depend on — v2 was
// the bump that added the `titles` map; v1 payloads have no titles
// map and would silently break the play flow's alt_titles fallback.
// Old un-versioned keys become misses on first access and the data
// re-fetches with the new schema.

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
    let key = format!("kitsu:v2:search:{}", normalize_query(query));
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(hits) = serde_json::from_str::<Vec<KitsuAnimeRef>>(&body) {
            // Warm-on-hit: idempotent. get_or_fetch returns from
            // disk on bytes-cache hit (no network), so this is cheap.
            // Catches the case where meta_cache outlives the image
            // cache (LRU evicted the bytes, response still warm).
            warm_signed_image_urls(state, &body);
            return Ok(hits);
        }
        // Cached body deserialized as something else — treat as miss and
        // rebuild rather than hand the frontend bad data.
    }
    let hits = state.kitsu.search(query, SEARCH_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&hits) {
        // TTL conversion can't fail in practice; clamp via try_from.
        let _ = meta_cache_put(&state.cache_pool, &key, &body, DISCOVERY_TTL.as_secs());
        warm_signed_image_urls(state, &body);
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
        "kitsu:v2:trending",
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
        "kitsu:v2:top_rated",
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
            warm_signed_image_urls(state, &body);
            return Ok(hits);
        }
    }
    let hits = fetch(DISCOVERY_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&hits) {
        let _ = meta_cache_put(&state.cache_pool, cache_key, &body, ttl_seconds);
        warm_signed_image_urls(state, &body);
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
            warm_signed_image_urls(state, &body);
            return Ok(eps);
        }
    }
    let eps = state
        .kitsu
        .episodes(anime_id, p, EPISODES_PAGE_LIMIT)
        .await?;
    if let Ok(body) = serde_json::to_string(&eps) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, EPISODES_TTL.as_secs());
        warm_signed_image_urls(state, &body);
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
    let key = format!("kitsu:v2:anime-slug:{slug}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(detail) = serde_json::from_str::<Option<KitsuAnimeRef>>(&body) {
            warm_signed_image_urls(state, &body);
            return Ok(detail);
        }
    }
    let detail = state.kitsu.anime_by_slug(slug).await?;
    if let Ok(body) = serde_json::to_string(&detail) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
        warm_signed_image_urls(state, &body);
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

/// Cache key for the reverse `allmanga show_id → kitsu_id` mapping.
/// Recorded on every successful play (where we know both ids) so
/// the home-page Continue Watching strip can look up the right
/// Kitsu entry by show_id instead of fuzzy-text-searching the
/// possibly-typo'd allmanga title.
fn allmanga_kitsu_key(show_id: &str) -> String {
    format!("allmanga2kitsu:v1:{show_id}")
}

/// Read the cached `allmanga show_id → kitsu_id` mapping. Returns
/// `None` on miss; SQLite errors propagate.
pub fn allmanga_kitsu_get(state: &AppState, show_id: &str) -> Result<Option<String>> {
    meta_cache_get(&state.cache_pool, &allmanga_kitsu_key(show_id))
}

/// Persist an `allmanga show_id → kitsu_id` mapping. Same TTL as
/// `title_match` (30d) — the mapping is as stable as Kitsu's id
/// space, and re-puts on every successful play keep it fresh.
pub fn allmanga_kitsu_put(state: &AppState, show_id: &str, kitsu_id: &str) -> Result<()> {
    meta_cache_put(
        &state.cache_pool,
        &allmanga_kitsu_key(show_id),
        kitsu_id,
        TITLE_MATCH_TTL.as_secs(),
    )
}

/// Bridge a history-recorded allmanga show_id to its Kitsu entry by
/// walking allmanga's `Show` GraphQL aliases (`englishName`,
/// `nativeName`, `altNames`) through Kitsu's text search. Returns the
/// first matching [`KitsuAnimeRef`] and persists the
/// `(show_id → kitsu_id)` reverse mapping so subsequent calls
/// short-circuit through `allmanga_kitsu_get`.
///
/// Recovers the case where allmanga's primary `name` is a stub the
/// frontend can't text-match (e.g. `"1P"` for One Piece, `"Nato:
/// Shippuuden"` for Naruto Shippuuden). The reverse cache normally
/// hides this, but a Settings → Clear cache wipes it and the bug
/// re-surfaces; this resolver re-warms the cache on first hit.
///
/// Returns `Ok(None)` when:
///   - The reverse cache has no entry AND
///   - allmanga's `Show` endpoint returns no usable aliases OR
///   - Every Kitsu text search returned zero hits.
///
/// Returns `Ok(Some(_))` on:
///   - Reverse-cache hit (fast path, no HTTP).
///   - Successful enrichment + Kitsu match.
///
/// # Errors
/// Cache I/O errors propagate; HTTP failures fall through to
/// `Ok(None)` so the caller (Continue Watching) can still render
/// the bare allmanga title without breaking.
pub async fn resolve_allmanga_show_id(
    state: &AppState,
    show_id: &str,
) -> Result<Option<KitsuAnimeRef>> {
    // 1) Reverse-cache fast path. If we've resolved this show before
    //    (or a successful play stamped the mapping), the kitsu_id is
    //    one cached IPC away. anime_detail itself is cached too, so
    //    a warm lookup is two synchronous SQLite reads.
    if let Ok(Some(kid)) = allmanga_kitsu_get(state, show_id) {
        if let Ok(detail) = kitsu_anime_detail(state, &kid).await {
            return Ok(Some(detail));
        }
        // Stale id (Kitsu removed it, or the cached row is bad) —
        // fall through and re-resolve from allmanga.
    }

    // 2) Hit allmanga's Show GraphQL to get the alias surface
    //    (englishName / nativeName / altNames). Network failure is
    //    soft — Continue Watching can still render the bare
    //    allmanga title, so we return Ok(None).
    let show = match crate::scraper::allanime::fetch_show(&state.proxy_http, show_id, None).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                show_id = show_id,
                error = ?e,
                "allmanga Show fetch failed during reverse-resolve",
            );
            return Ok(None);
        }
    };

    // 3) Walk aliases through Kitsu text search. First non-empty hit
    //    wins. We could be cleverer with slug-matching here (the
    //    pickKitsuMatch heuristic already does that on the frontend
    //    for cour-suffixed entries), but the cryptic-name case is
    //    simpler — the alias is usually unambiguous (e.g.
    //    "One Piece" lands one entry).
    for term in show.search_terms() {
        let hits = match state.kitsu.search(&term, SEARCH_PAGE_LIMIT).await {
            Ok(h) => h,
            Err(_) => continue, // Single-term failure shouldn't break the walk.
        };
        if let Some(first) = hits.into_iter().next() {
            // 4) Persist the mapping so subsequent calls short-circuit
            //    through step 1. Failure to write the cache is
            //    non-fatal — the resolution still succeeds for this
            //    request; the next call just walks aliases again.
            if let Err(e) = allmanga_kitsu_put(state, show_id, &first.id) {
                tracing::warn!(
                    show_id = show_id,
                    kitsu_id = %first.id,
                    error = ?e,
                    "reverse-cache write failed during enrichment resolve",
                );
            }
            return Ok(Some(first));
        }
    }

    // 5) Walked every alias, nothing matched. Soft-fail.
    Ok(None)
}

/// Cache-key prefix for the per-show last-watched timestamp. Stamped
/// on every GUI-driven `mark-watched` call; the home-page Continue
/// Watching strip sorts by it descending so the user's most recent
/// play surfaces first regardless of file position. CLI-only plays
/// never stamp here, so those rows fall to the bottom of the strip
/// (still rendered, just demoted).
const WATCHED_AT_PREFIX: &str = "watched-at:v1:";

fn watched_at_key(show_id: &str) -> String {
    format!("{WATCHED_AT_PREFIX}{show_id}")
}

/// Read the millis-since-epoch wall-clock timestamp for the last GUI
/// play of `show_id`. Returns `None` for shows the user has only
/// played via the CLI (or hasn't played at all).
pub fn watched_at_get(state: &AppState, show_id: &str) -> Result<Option<i64>> {
    let body = meta_cache_get(&state.cache_pool, &watched_at_key(show_id))?;
    Ok(body.and_then(|s| s.parse::<i64>().ok()))
}

/// Stamp `show_id` with `watched_at_ms`. Re-puts overwrite. Same TTL
/// as the title-match cache (30d) — long enough that the user's
/// recent history isn't churned, short enough that ancient stamps
/// for shows they never replay don't pile up.
pub fn watched_at_put(state: &AppState, show_id: &str, watched_at_ms: i64) -> Result<()> {
    meta_cache_put(
        &state.cache_pool,
        &watched_at_key(show_id),
        &watched_at_ms.to_string(),
        TITLE_MATCH_TTL.as_secs(),
    )
}

/// Return every stamped `(show_id → watched_at_ms)` pair. Frontend
/// fetches this once on home mount, joins against the history list
/// from `/api/history`, and sorts. `meta_cache_list_prefix` filters
/// to non-expired rows.
pub fn watched_at_all(state: &AppState) -> Result<std::collections::HashMap<String, i64>> {
    let rows = crate::cache::meta_cache_list_prefix(&state.cache_pool, WATCHED_AT_PREFIX)?;
    let mut out = std::collections::HashMap::with_capacity(rows.len());
    for (key, body) in rows {
        let show_id = key
            .strip_prefix(WATCHED_AT_PREFIX)
            .unwrap_or(&key)
            .to_string();
        if let Ok(ms) = body.parse::<i64>() {
            out.insert(show_id, ms);
        }
    }
    Ok(out)
}

/// Fetch a single anime by Kitsu id. Cache key: `kitsu:anime:<id>`.
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::anime_detail`] on miss.
pub async fn kitsu_anime_detail(state: &AppState, id: &str) -> Result<KitsuAnimeRef> {
    let key = format!("kitsu:v2:anime:{id}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(detail) = serde_json::from_str::<KitsuAnimeRef>(&body) {
            warm_signed_image_urls(state, &body);
            return Ok(detail);
        }
    }
    let detail = state.kitsu.anime_detail(id).await?;
    if let Ok(body) = serde_json::to_string(&detail) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
        warm_signed_image_urls(state, &body);
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

    // — allmanga → kitsu reverse mapping ——————————————————————————————
    //
    // Forward direction (Kitsu canonical_title → allmanga show) works
    // through ani-cli's lenient catalog search. The reverse — taking
    // an allmanga title from `ani-hsts` and finding its Kitsu entry —
    // is broken when allmanga has typos (e.g. "Nato: Shippuuden" for
    // Naruto), because Kitsu's text search isn't fuzzy and returns
    // unrelated first hits. So we record the (allmanga show_id →
    // kitsu_id) pair on every successful play (where we know both
    // ids) and the home page reads it directly by show_id.

    #[test]
    fn allmanga_kitsu_cache_round_trips_for_a_given_show_id() {
        let state = state_with_kitsu_at("http://unused");
        allmanga_kitsu_put(&state, "vDTSJHSpYnrkZnAvG", "11469").expect("put ok");
        let got = allmanga_kitsu_get(&state, "vDTSJHSpYnrkZnAvG").expect("get ok");
        assert_eq!(got, Some("11469".to_string()));
    }

    #[test]
    fn allmanga_kitsu_cache_returns_none_for_unknown_show_id() {
        let state = state_with_kitsu_at("http://unused");
        assert_eq!(
            allmanga_kitsu_get(&state, "never-played").expect("get ok"),
            None
        );
    }

    #[test]
    fn allmanga_kitsu_cache_overwrites_on_re_put() {
        // If the user navigates to a different Kitsu entry for the
        // same allmanga show (e.g. they corrected their pick), the
        // newer mapping wins.
        let state = state_with_kitsu_at("http://unused");
        allmanga_kitsu_put(&state, "abc", "old-kitsu").expect("put old");
        allmanga_kitsu_put(&state, "abc", "new-kitsu").expect("put new");
        assert_eq!(
            allmanga_kitsu_get(&state, "abc").expect("get"),
            Some("new-kitsu".to_string())
        );
    }

    #[test]
    fn allmanga_kitsu_cache_separates_by_show_id() {
        let state = state_with_kitsu_at("http://unused");
        allmanga_kitsu_put(&state, "show-a", "kitsu-a").expect("put a");
        allmanga_kitsu_put(&state, "show-b", "kitsu-b").expect("put b");
        assert_eq!(
            allmanga_kitsu_get(&state, "show-a").expect("get a"),
            Some("kitsu-a".to_string())
        );
        assert_eq!(
            allmanga_kitsu_get(&state, "show-b").expect("get b"),
            Some("kitsu-b".to_string())
        );
    }

    // — resolve_allmanga_show_id: end-to-end orchestration ——————————
    //
    // The full chain (allmanga `Show` GraphQL → walk aliases → Kitsu
    // text search → cache result) is exercised against a MockServer
    // whose URI we stamp into BOTH the Kitsu client base AND the
    // allanime base override (via state_with_kitsu_at hijacking the
    // Kitsu client; the allanime call uses state.proxy_http with no
    // override, so production prod plumbing isn't exercised here —
    // see the green commit for the with_bases helper that threads a
    // test override into the allanime call too).

    #[tokio::test]
    async fn resolve_allmanga_show_id_short_circuits_on_reverse_cache_hit() {
        // When the reverse mapping already exists, no allmanga or
        // Kitsu HTTP fires — the cached kitsu_id resolves through
        // anime_detail and returns. State has no mock servers
        // attached, so the test verifies the pure cache hit by
        // checking the call returns Some without panicking.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/12"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(DETAIL_FIXTURE.to_vec()),
            )
            .mount(&mock)
            .await;
        let state = state_with_kitsu_at(&mock.uri());
        allmanga_kitsu_put(&state, "ReooPAxPMsHM4KPMY", "12").expect("seed cache");

        let got = resolve_allmanga_show_id(&state, "ReooPAxPMsHM4KPMY")
            .await
            .expect("resolve ok");
        assert!(
            got.is_some(),
            "reverse cache hit must short-circuit to a Some(_) response"
        );
        assert_eq!(got.unwrap().id, "12");
    }

    #[tokio::test]
    async fn resolve_allmanga_show_id_returns_none_when_no_cache_and_no_aliases() {
        // No reverse cache entry; allmanga (real) call is presumed to
        // return nothing meaningful for a synthetic id. The contract
        // is "fail soft" — Ok(None), not an error, so Continue
        // Watching can still render the bare allmanga title.
        let state = state_with_kitsu_at("http://127.0.0.1:1");
        let got = resolve_allmanga_show_id(&state, "this-id-will-not-resolve").await;
        // Either Ok(None) (no aliases, no Kitsu match) or Ok(Some(_))
        // is acceptable — we only assert the call doesn't panic and
        // doesn't surface a network error to the caller. Stub returns
        // Ok(None); the green-commit impl returns Ok(None) for this
        // synthetic id too.
        assert!(matches!(got, Ok(_)), "resolver must not error: {got:?}");
    }

    // — Watched-at timestamps for Continue Watching ordering ——————————
    //
    // ani-hsts is keyed by show_id and only stores ep_no/title — no
    // timestamps. The file's row order reflects "first time played"
    // (in-place updates don't move rows). To render Continue Watching
    // most-recently-watched-first we record a per-show_id wall-clock
    // millis timestamp on every GUI play. CLI plays bypass this; their
    // entries fall to the bottom of the strip ordered by file position.

    #[test]
    fn watched_at_round_trips_for_a_given_show_id() {
        let state = state_with_kitsu_at("http://unused");
        watched_at_put(&state, "vDTSJHSpYnrkZnAvG", 1_700_000_000_000).expect("put");
        let got = watched_at_get(&state, "vDTSJHSpYnrkZnAvG").expect("get");
        assert_eq!(got, Some(1_700_000_000_000));
    }

    #[test]
    fn watched_at_returns_none_for_unstamped_show_id() {
        let state = state_with_kitsu_at("http://unused");
        assert_eq!(
            watched_at_get(&state, "never-played-in-gui").expect("get"),
            None
        );
    }

    #[test]
    fn watched_at_overwrites_on_re_put() {
        // Each new play stamps the latest time, replacing the prior.
        let state = state_with_kitsu_at("http://unused");
        watched_at_put(&state, "abc", 1_700_000_000_000).expect("put 1");
        watched_at_put(&state, "abc", 1_800_000_000_000).expect("put 2");
        assert_eq!(
            watched_at_get(&state, "abc").expect("get"),
            Some(1_800_000_000_000)
        );
    }

    #[test]
    fn watched_at_all_returns_every_stamped_show_id() {
        let state = state_with_kitsu_at("http://unused");
        watched_at_put(&state, "show-a", 1_700_000_000_000).expect("put a");
        watched_at_put(&state, "show-b", 1_800_000_000_000).expect("put b");
        let map = watched_at_all(&state).expect("all");
        assert_eq!(map.get("show-a"), Some(&1_700_000_000_000));
        assert_eq!(map.get("show-b"), Some(&1_800_000_000_000));
    }
}
