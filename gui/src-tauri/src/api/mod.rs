//! HTTP API exposed by the localhost server.
//!
//! Wraps the same `commands::*` functions Tauri's `#[tauri::command]`
//! handlers used to call. The migration from Tauri (webkit2gtk) to
//! Electron (Chromium) means the frontend talks to this router via
//! `fetch` instead of `invoke`. Routes mirror the IPC surface 1:1 in
//! shape; only the wire protocol changes.
//!
//! Mounted alongside the streaming-proxy router (`crate::proxy`) on
//! the same kernel-assigned loopback port. Both routers share the
//! axum app, both bind 127.0.0.1 only.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;

use crate::app::AppState;
use crate::commands::{
    app_info, external_player, history as h_inner, kitsu as kitsu_inner, proxy_url,
    session as session_inner, settings as settings_inner,
};
use crate::config::Config;
use crate::error::AniError;
use crate::history::HistoryEntry;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuEpisode};

/// Map every `AniError` variant to the closest matching HTTP status.
/// The body is the same JSON shape Tauri used to surface (a `kind`
/// discriminator + optional `key` / `detail`), so the frontend
/// error-handling code keeps the same structure as it switches from
/// `invoke()` rejection payloads to `fetch()` 4xx/5xx bodies.
impl IntoResponse for AniError {
    fn into_response(self) -> Response {
        let status = match self {
            AniError::NoResults => StatusCode::NOT_FOUND,
            AniError::InvalidToken => StatusCode::UNAUTHORIZED,
            AniError::Upstream { .. } => StatusCode::BAD_GATEWAY,
            AniError::Network => StatusCode::SERVICE_UNAVAILABLE,
            AniError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            AniError::ParseFailed { .. }
            | AniError::MissingBinary
            | AniError::Cache
            | AniError::Io
            | AniError::Config
            | AniError::Metadata
            | AniError::Scraper { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

/// Build the API router. Returns a `Router<()>` (state already
/// applied via `with_state`) so it can be `merge`d with the proxy
/// router at startup.
pub fn build_api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/app-info", get(get_app_info))
        .route("/api/proxy-base-url", get(get_proxy_base_url))
        .route("/api/history", get(get_history).delete(delete_history))
        .route("/api/external-player", post(post_external_player))
        .route("/api/sessions", post(post_session))
        .route("/api/kitsu/search", post(post_kitsu_search))
        .route("/api/kitsu/anime/:id", get(get_kitsu_anime_detail))
        .route(
            "/api/kitsu/anime-by-slug/:slug",
            get(get_kitsu_anime_by_slug),
        )
        .route("/api/kitsu/trending", get(get_kitsu_trending))
        .route("/api/kitsu/top-rated", get(get_kitsu_top_rated))
        .route("/api/kitsu/episodes/:anime_id", get(get_kitsu_episodes))
        .route(
            "/api/title-match",
            get(get_title_match).put(put_title_match),
        )
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/cache", delete(delete_cache))
        .route("/api/image", get(get_image))
        .with_state(state)
        // The Electron renderer in dev runs at `http://localhost:<vite>`
        // while we bind 127.0.0.1:<random> — that's cross-origin, so
        // every response (and every preflight) needs permissive CORS
        // headers. Safe at the loopback boundary: the listener never
        // accepts non-loopback connections, so `*` here only opens the
        // door to other apps on the same machine, not the internet.
        .layer(CorsLayer::permissive())
}

// — Handlers ——————————————————————————————————————————————————————————

async fn get_app_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<app_info::AppInfo>, AniError> {
    Ok(Json(app_info::app_info(&state)?))
}

async fn get_proxy_base_url(State(state): State<Arc<AppState>>) -> Result<Json<String>, AniError> {
    Ok(Json(proxy_url::proxy_base_url(&state)?))
}

async fn get_history(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<HistoryEntry>>, AniError> {
    Ok(Json(h_inner::history_list(&state)?))
}

async fn delete_history(State(state): State<Arc<AppState>>) -> Result<StatusCode, AniError> {
    h_inner::history_clear(&state)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_external_player(
    Json(args): Json<external_player::LaunchArgs>,
) -> Result<StatusCode, AniError> {
    external_player::open_external_player(&args)?;
    Ok(StatusCode::ACCEPTED)
}

async fn post_session(
    State(state): State<Arc<AppState>>,
    Json(args): Json<session_inner::CreateSessionArgs>,
) -> Result<Json<session_inner::CreateSessionResponse>, AniError> {
    Ok(Json(session_inner::create_session(&state, &args)?))
}

#[derive(Deserialize)]
struct SearchBody {
    query: String,
}

async fn post_kitsu_search(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SearchBody>,
) -> Result<Json<Vec<KitsuAnimeRef>>, AniError> {
    Ok(Json(kitsu_inner::kitsu_search(&state, &body.query).await?))
}

async fn get_kitsu_anime_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<KitsuAnimeRef>, AniError> {
    Ok(Json(kitsu_inner::kitsu_anime_detail(&state, &id).await?))
}

async fn get_kitsu_anime_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<Option<KitsuAnimeRef>>, AniError> {
    Ok(Json(kitsu_inner::kitsu_anime_by_slug(&state, &slug).await?))
}

async fn get_kitsu_trending(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<KitsuAnimeRef>>, AniError> {
    Ok(Json(kitsu_inner::kitsu_trending(&state).await?))
}

async fn get_kitsu_top_rated(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<KitsuAnimeRef>>, AniError> {
    Ok(Json(kitsu_inner::kitsu_top_rated(&state).await?))
}

#[derive(Deserialize)]
struct EpisodesQuery {
    page: Option<u32>,
}

async fn get_kitsu_episodes(
    State(state): State<Arc<AppState>>,
    Path(anime_id): Path<String>,
    Query(q): Query<EpisodesQuery>,
) -> Result<Json<Vec<KitsuEpisode>>, AniError> {
    let page = q.page.unwrap_or(1);
    Ok(Json(
        kitsu_inner::kitsu_episodes(&state, &anime_id, page).await?,
    ))
}

#[derive(Deserialize)]
struct TitleMatchQuery {
    title: String,
    cour: u32,
}

async fn get_title_match(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TitleMatchQuery>,
) -> Result<Json<Option<String>>, AniError> {
    Ok(Json(kitsu_inner::title_match_get(
        &state, &q.title, q.cour,
    )?))
}

#[derive(Deserialize)]
struct TitleMatchBody {
    title: String,
    cour: u32,
    kitsu_id: String,
}

async fn put_title_match(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TitleMatchBody>,
) -> Result<StatusCode, AniError> {
    kitsu_inner::title_match_put(&state, &body.title, body.cour, &body.kitsu_id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Result<Json<Config>, AniError> {
    Ok(Json(settings_inner::settings_get(&state)?))
}

async fn put_settings(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<Config>,
) -> Result<StatusCode, AniError> {
    settings_inner::settings_put(&state, &cfg)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_cache(State(state): State<Arc<AppState>>) -> Result<StatusCode, AniError> {
    crate::cache::meta_cache_clear(&state.cache_pool)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ImageQuery {
    url: String,
}

/// Serve a cached/freshly-fetched image. The Tauri build used a custom
/// `image://` URI scheme; under Electron the renderer can't reach
/// that, so it asks for the bytes over plain HTTP. Same on-disk cache
/// (`meta::images`) backs both transports.
///
/// Only `https://` upstreams are accepted — refusing other schemes
/// avoids letting a malicious renderer turn the loopback server into
/// an SSRF springboard for `file://`, `http://localhost:*`, etc.
async fn get_image(State(state): State<Arc<AppState>>, Query(q): Query<ImageQuery>) -> Response {
    if !q.url.starts_with("https://") {
        return (StatusCode::BAD_REQUEST, "url must be https://").into_response();
    }
    match crate::meta::images::get_or_fetch(&state.proxy_http, &state.image_cache_dir, &q.url).await
    {
        Ok((bytes, mime)) => (
            StatusCode::OK,
            [
                (axum::http::header::CONTENT_TYPE, mime),
                // 24h is enough — Kitsu/AniList CDN URLs are stable
                // per anime, and the on-disk cache makes refetches
                // cheap if a header eviction happens early.
                (axum::http::header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SCRAPER_CONCURRENCY;
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Semaphore;
    use tower::ServiceExt;

    /// Build a stub AppState for router tests. Uses an in-memory SQLite
    /// pool, a tempdir for image cache + config, and a Kitsu client
    /// pointing at an unused base URL (tests that don't need Kitsu
    /// won't touch the network).
    fn test_app_state(td: &TempDir) -> AppState {
        let kitsu_base = "http://127.0.0.1:1"; // never reached by these tests
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: PathBuf::from("/tmp/ani-cli"),
            history_path: td.path().join("ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
            image_cache_dir: td.path().join("images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::with_base(reqwest::Client::new(), kitsu_base),
            config_path: td.path().join("config.toml"),
        }
    }

    async fn body_string(resp: Response) -> String {
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("utf-8")
    }

    #[tokio::test]
    async fn get_app_info_returns_200_with_app_info_shape() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/app-info")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        // Shape sanity — actual values come from the shared AppState
        // and don't matter for this test.
        assert!(body.contains("version"), "body: {body}");
        assert!(body.contains("ani_cli_path"), "body: {body}");
        assert!(body.contains("proxy_base_url"), "body: {body}");
    }

    #[tokio::test]
    async fn get_proxy_base_url_returns_a_loopback_url() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/proxy-base-url")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        // ProxyOrigin::new("127.0.0.1", 12345) → "http://127.0.0.1:12345"
        assert!(body.contains("127.0.0.1"), "body: {body}");
    }

    #[tokio::test]
    async fn get_history_with_no_hsts_file_returns_empty_list() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/history")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert_eq!(body.trim(), "[]");
    }

    #[tokio::test]
    async fn delete_history_returns_204() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/history")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn get_settings_returns_default_config_when_file_absent() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/settings")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("locale"), "body: {body}");
        assert!(body.contains("mode"), "body: {body}");
    }

    #[tokio::test]
    async fn put_settings_persists_then_get_returns_the_new_value() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let put = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"locale":"pt-BR","mode":"dub","quality":"1080","external_player":"vlc","image_cache_cap_mb":1000}"#,
                    ))
                    .expect("req"),
            )
            .await
            .expect("oneshot");
        assert_eq!(put.status(), StatusCode::NO_CONTENT);

        let get = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/settings")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");
        assert_eq!(get.status(), StatusCode::OK);
        let body = body_string(get).await;
        assert!(body.contains("\"locale\":\"pt-BR\""), "body: {body}");
        assert!(body.contains("\"mode\":\"dub\""), "body: {body}");
    }

    #[tokio::test]
    async fn delete_cache_returns_204() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/cache")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn put_then_get_title_match_round_trips() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let put = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/title-match")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"title":"Stone Ocean Part 2","cour":2,"kitsu_id":"46010"}"#,
                    ))
                    .expect("req"),
            )
            .await
            .expect("oneshot");
        assert_eq!(put.status(), StatusCode::NO_CONTENT);

        let get = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/title-match?title=Stone+Ocean+Part+2&cour=2")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");
        assert_eq!(get.status(), StatusCode::OK);
        let body = body_string(get).await;
        assert!(body.contains("46010"), "body: {body}");
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/does-not-exist")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn ani_error_no_results_serializes_with_kind() {
        // Spot-check the IntoResponse impl — the body has the same JSON
        // shape Tauri used to deliver as the rejection payload, so the
        // frontend's error parser can keep the same structure.
        let resp = AniError::NoResults.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_string(resp).await;
        assert!(body.contains("\"kind\":\"no_results\""), "body: {body}");
    }

    /// In Electron-dev the renderer lives on `http://localhost:<vite>`
    /// while the backend binds 127.0.0.1:<random>. Browsers treat that
    /// as cross-origin, so every response must carry permissive CORS
    /// headers — otherwise the renderer's `fetch()` calls fail with no
    /// useful error. The packaged build (file:// origin) needs the same
    /// headers for the same reason.
    #[tokio::test]
    async fn cors_get_response_carries_acao_for_external_origin() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/app-info")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        let acao = response
            .headers()
            .get("access-control-allow-origin")
            .map(|v| v.to_str().unwrap_or("").to_string());
        assert!(
            acao.is_some(),
            "missing access-control-allow-origin header; got headers: {:?}",
            response.headers()
        );
    }

    #[tokio::test]
    async fn cors_preflight_options_returns_2xx_with_allow_methods() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/kitsu/search")
                    .header("origin", "http://localhost:5173")
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "content-type")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert!(
            response.status().is_success(),
            "preflight failed: {}",
            response.status()
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin"),
            "missing access-control-allow-origin"
        );
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-methods"),
            "missing access-control-allow-methods"
        );
    }

    /// Cross-origin `<img src=>` loads can't ride Tauri's `image://`
    /// custom protocol from Electron — the renderer fetches via plain
    /// HTTP. The route must serve cached bytes with the right
    /// Content-Type, and refuse non-https upstreams (defense in depth
    /// against an XSS asking the loopback server to fetch arbitrary
    /// schemes).
    #[tokio::test]
    async fn image_route_serves_cached_bytes_with_content_type() {
        let td = TempDir::new().expect("tempdir");
        let state = test_app_state(&td);
        // Pre-populate the cache so the route doesn't try to fetch
        // upstream during the test.
        let upstream = "https://media.kitsu.app/anime/12/poster.jpg";
        let hash = crate::meta::images::hash_url(upstream);
        let on_disk = state.image_cache_dir.join(&hash[..2]);
        std::fs::create_dir_all(&on_disk).expect("mkdir cache bucket");
        std::fs::write(
            on_disk.join(format!("{hash}.jpg")),
            b"\xff\xd8\xff\xe0fake-jpeg",
        )
        .expect("seed cache");

        let router = build_api_router(Arc::new(state));
        // %3A %2F are `:` and `/`. URL is short and stable so hardcoding
        // the encoded form is clearer than pulling in a serde-urlencoded
        // helper just for the test.
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/image?url=https%3A%2F%2Fmedia.kitsu.app%2Fanime%2F12%2Fposter.jpg")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("image/jpeg")
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes();
        assert_eq!(body.as_ref(), b"\xff\xd8\xff\xe0fake-jpeg");
    }

    #[tokio::test]
    async fn image_route_rejects_non_https_url() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/image?url=http%3A%2F%2Finsecure.example%2Fx.jpg")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn image_route_rejects_missing_url_param() {
        let td = TempDir::new().expect("tempdir");
        let router = build_api_router(Arc::new(test_app_state(&td)));
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/image")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");

        // axum's Query extractor rejects with 400 for missing required fields.
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
