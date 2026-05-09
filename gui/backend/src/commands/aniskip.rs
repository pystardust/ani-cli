//! aniskip command — bridges Kitsu id → MAL id → aniskip skip
//! times, cached so subsequent visits skip both round-trips.
//!
//! The frontend player uses this on `loadedmetadata` to learn
//! when to render the Skip OP / Skip Outro overlay buttons.

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;
use crate::meta::aniskip::SkipInterval;

/// 7 days. Skip times stabilize quickly — a few days of crowd
/// edits per episode then they hold steady.
const ANISKIP_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Fetch the aniskip skip-time list for a given Kitsu id +
/// episode + episode length (seconds). Resolves the MAL id
/// transparently via Kitsu's mappings; caches the result keyed
/// by `(mal_id, episode)` so repeat visits to the same episode
/// reuse the lookup.
///
/// Returns `Ok(empty Vec)` when:
///   - Kitsu has no MAL mapping for this anime, or
///   - aniskip has no skip times catalogued for this episode.
/// Either is normal — the player just doesn't show the skip
/// button.
///
/// # Errors
/// Network / Upstream / ParseFailed from the underlying clients.
pub async fn aniskip_get(
    state: &AppState,
    kitsu_id: &str,
    episode: &str,
    episode_length: f32,
) -> Result<Vec<SkipInterval>> {
    // Bridge kitsu_id → mal_id. No mapping = aniskip can't index
    // it; return empty so the player skips rendering the button.
    let mal_id = match state.kitsu.mal_id_for_kitsu_id(kitsu_id).await {
        Ok(Some(id)) => id,
        Ok(None) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let key = cache_key(mal_id, episode);
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(intervals) = serde_json::from_str::<Vec<SkipInterval>>(&body) {
            return Ok(intervals);
        }
        // Corrupt cache row — fall through to refetch.
    }

    let intervals = crate::meta::aniskip::fetch_skip_times(
        &state.proxy_http,
        mal_id,
        episode,
        episode_length,
        None,
    )
    .await?;

    if let Ok(body) = serde_json::to_string(&intervals) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANISKIP_TTL_SECS);
    }
    Ok(intervals)
}

/// Cache key for `(mal_id, episode)` lookups. Schema v1.
fn cache_key(mal_id: u32, episode: &str) -> String {
    format!("aniskip:v1:{mal_id}:{episode}")
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    const ANIME_WITHOUT_MAPPINGS: &str = r##"{
        "data": { "id": "999", "type": "anime",
            "attributes": { "canonicalTitle": "Other Show" } },
        "included": []
    }"##;

    /// No MAL mapping = no path forward; aniskip is keyed on MAL
    /// ids exclusively. Player should fall back to no skip
    /// button. Behaviorally important: NEVER raise an error here;
    /// it'd make the player bubble a "couldn't load skip times"
    /// state on every show that's not on MAL.
    #[tokio::test]
    async fn returns_empty_when_kitsu_has_no_mal_mapping() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/999"))
            .respond_with(ResponseTemplate::new(200).set_body_string(ANIME_WITHOUT_MAPPINGS))
            .mount(&server)
            .await;
        let state = state_with_kitsu_at(&server.uri());
        let v = aniskip_get(&state, "999", "1", 1440.0).await.expect("ok");
        assert!(v.is_empty());
    }

    /// Cache short-circuit: a fresh row in meta_cache should be
    /// returned without touching Kitsu or aniskip. Asserts the
    /// cache key shape stays stable under refactors.
    #[tokio::test]
    async fn cache_hit_returns_stored_intervals_without_network() {
        let state = state_with_kitsu_at("http://unused");
        // Pre-populate the cache for (mal=21, ep=1) — the format
        // matches what the green impl will write. The Kitsu
        // mappings call is needed first to learn the MAL id
        // (we go kitsu_id → mal_id → cache_key), so we also
        // need to mock that. Use a separate server for that
        // hop.
        let kitsu = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/12"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r##"{"data":{"id":"12","type":"anime","attributes":{"canonicalTitle":"X"}},"included":[{"id":"m","type":"mappings","attributes":{"externalSite":"myanimelist/anime","externalId":"21"}}]}"##,
            ))
            .mount(&kitsu)
            .await;
        let state = AppState {
            kitsu: KitsuClient::with_base(reqwest::Client::new(), kitsu.uri()),
            ..state
        };
        // Pre-populate the cache row so the green impl's hit path
        // returns these without ever hitting aniskip.com.
        let intervals = vec![SkipInterval {
            skip_type: "op".into(),
            start_time: 5.0,
            end_time: 90.0,
        }];
        let body = serde_json::to_string(&intervals).expect("serialize");
        crate::cache::meta_cache_put(&state.cache_pool, "aniskip:v1:21:1", &body, 3600).unwrap();

        let v = aniskip_get(&state, "12", "1", 1440.0).await.expect("ok");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].skip_type, "op");
        assert_eq!(v[0].start_time, 5.0);
    }

    #[test]
    fn cache_key_includes_mal_and_episode() {
        // Stable key shape — the lookup chain depends on it.
        assert_eq!(cache_key(21, "1"), "aniskip:v1:21:1");
        assert_eq!(cache_key(59970, "12"), "aniskip:v1:59970:12");
    }
}
