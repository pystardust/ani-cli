//! Local stream proxy for the embedded `<video>` + hls.js player.
//!
//! Bound to `127.0.0.1:0` (kernel-assigned port) at app startup. Routes:
//!
//! - `GET /healthz` — liveness probe used by the frontend bootstrap.
//! - `GET /s/<session>/master.m3u8` — fetch + rewrite + return master.
//! - `GET /s/<session>/seg?u=<base64-url>&t=<hmac>` — proxy a segment.
//! - `GET /s/<session>/sub.vtt` — proxy the subtitle file.
//!
//! Every fetch upstream uses the [`StreamSession`]'s stored `Referer:`
//! header. Segment URLs in rewritten manifests carry an HMAC signature
//! the proxy verifies before issuing the upstream fetch.

pub mod m3u8;
pub mod token;
pub mod upstream;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use base64::Engine;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

use crate::error::AniError;

pub use m3u8::{rewrite_master, rewrite_media, ProxyOrigin};
pub use token::{
    sign_segment, verify_segment, AppSecret, MediaKind, SessionId, SessionTable, StreamSession,
};

/// Shared state every proxy route reads.
#[derive(Clone)]
pub struct ProxyState {
    /// Live stream sessions.
    pub sessions: SessionTable,
    /// Per-process HMAC key used for segment tokens.
    pub secret: AppSecret,
    /// Outbound `reqwest` client (upstream fetches).
    pub client: reqwest::Client,
    /// How rewritten URIs are formatted back into manifests. Set after the
    /// proxy actually binds to a port.
    pub origin: ProxyOrigin,
}

/// Build the axum router. The router is generic over its state, so callers
/// can swap states between tests and prod without re-wiring routes.
pub fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/s/:session/master.m3u8", get(handle_master))
        .route("/s/:session/file.mp4", get(handle_mp4))
        .route("/s/:session/seg", get(handle_seg))
        .route("/s/:session/sub.vtt", get(handle_sub))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(Arc::new(state))
}

/// Bind a TCP listener on `127.0.0.1` to the requested port (use `0` for
/// kernel-assigned). Returns the actual `SocketAddr` and the listener so
/// the caller can pass both to `axum::serve` on its own runtime.
///
/// # Errors
/// Returns [`AniError::Network`] when the bind fails.
pub async fn bind_loopback(port: u16) -> crate::Result<(SocketAddr, tokio::net::TcpListener)> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| AniError::Network)?;
    let bound = listener.local_addr().map_err(|_| AniError::Network)?;
    Ok((bound, listener))
}

#[allow(clippy::unused_async)]
async fn healthz() -> &'static str {
    "ok"
}

async fn handle_master(
    State(state): State<Arc<ProxyState>>,
    Path(session_str): Path<String>,
) -> Response {
    let session = match SessionId::parse(&session_str) {
        Ok(s) => s,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid session id"),
    };

    let Some(sess) = state.sessions.get(&session) else {
        return error_response(StatusCode::NOT_FOUND, "session not found or expired");
    };

    // The HLS rewrite path only makes sense for .m3u8 sessions; an MP4
    // would otherwise be buffered (hundreds of MB) and fail to parse.
    // 415 tells the renderer to use /file.mp4 instead.
    if !matches!(sess.media_kind, MediaKind::Hls) {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "session media is not HLS — use /file.mp4",
        );
    }

    let body = match upstream::fetch_text(&state.client, &sess.upstream_url, &sess.referer).await {
        Ok((bytes, _ct)) => bytes,
        Err(AniError::Upstream { status }) => {
            return error_response(
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                "upstream error",
            );
        }
        Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream fetch failed"),
    };

    let rewritten = match rewrite_master(
        &body,
        &sess.upstream_url,
        &state.origin,
        session,
        &state.secret,
    ) {
        Ok(s) => s,
        Err(_) => {
            // Try as a media playlist before giving up — some upstreams
            // return media directly when there's only one variant.
            match rewrite_media(
                &body,
                &sess.upstream_url,
                &state.origin,
                session,
                &state.secret,
            ) {
                Ok(s) => s,
                Err(_) => {
                    return error_response(
                        StatusCode::BAD_GATEWAY,
                        "upstream manifest unparseable",
                    );
                }
            }
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/vnd.apple.mpegurl"),
    );
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store"),
    );
    (StatusCode::OK, headers, rewritten).into_response()
}

/// Streaming pass-through for direct-MP4 upstreams (wixmp/sharepoint
/// /fast4speed). The proxy doesn't parse, rewrite, or buffer the body —
/// it forwards the inbound `Range` header upstream and pipes the
/// response back chunk-by-chunk so the renderer's `<video>` element
/// can seek without downloading the full file.
async fn handle_mp4(
    State(state): State<Arc<ProxyState>>,
    Path(session_str): Path<String>,
    headers_in: HeaderMap,
) -> Response {
    let session = match SessionId::parse(&session_str) {
        Ok(s) => s,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid session id"),
    };
    let Some(sess) = state.sessions.get(&session) else {
        return error_response(StatusCode::NOT_FOUND, "session not found or expired");
    };
    if !matches!(sess.media_kind, MediaKind::Mp4) {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "session media is not MP4 — use /master.m3u8",
        );
    }

    let range = headers_in
        .get(axum::http::header::RANGE)
        .and_then(|v| v.to_str().ok());

    let upstream_resp =
        match upstream::fetch_streaming(&state.client, &sess.upstream_url, &sess.referer, range)
            .await
        {
            Ok(r) => r,
            Err(AniError::Upstream { status }) => {
                return error_response(
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                    "upstream error",
                );
            }
            Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream fetch failed"),
        };

    // Echo back the upstream status (200 for full, 206 for partial)
    // and the headers a video element needs: content-type tells the
    // renderer how to decode, content-length / content-range / accept-
    // ranges drive the seek bar. Other headers stay upstream-side.
    let status =
        StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut out_headers = HeaderMap::new();
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "last-modified",
        "etag",
    ] {
        if let Some(v) = upstream_resp.headers().get(name) {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_bytes(v.as_bytes()),
            ) {
                out_headers.insert(name, value);
            }
        }
    }
    out_headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store"),
    );

    let body = Body::from_stream(upstream_resp.bytes_stream());
    (status, out_headers, body).into_response()
}

#[derive(Debug, Deserialize)]
struct SegmentQuery {
    /// base64url-encoded original (upstream) URL.
    u: String,
    /// HMAC signature.
    t: String,
}

async fn handle_seg(
    State(state): State<Arc<ProxyState>>,
    Path(session_str): Path<String>,
    Query(q): Query<SegmentQuery>,
    headers_in: HeaderMap,
) -> Response {
    let session = match SessionId::parse(&session_str) {
        Ok(s) => s,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid session id"),
    };
    let Some(sess) = state.sessions.get(&session) else {
        return error_response(StatusCode::NOT_FOUND, "session not found");
    };

    let upstream_url = match decode_seg_url(&q.u) {
        Ok(u) => u,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "bad segment url encoding"),
    };

    if verify_segment(&state.secret, session, upstream_url.as_str(), &q.t).is_err() {
        return error_response(StatusCode::FORBIDDEN, "invalid segment token");
    }

    // For .m3u8 sub-playlists, fetch + rewrite. For raw segments (mp4/ts/m4s),
    // stream bytes through with the Range header preserved.
    let path = upstream_url.path();
    let is_manifest = path.ends_with(".m3u8");

    if is_manifest {
        let body = match upstream::fetch_text(&state.client, &upstream_url, &sess.referer).await {
            Ok((b, _ct)) => b,
            Err(AniError::Upstream { status }) => {
                return error_response(
                    StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                    "upstream",
                );
            }
            Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream fetch failed"),
        };
        let rewritten =
            match rewrite_media(&body, &upstream_url, &state.origin, session, &state.secret) {
                Ok(s) => s,
                Err(_) => {
                    return error_response(
                        StatusCode::BAD_GATEWAY,
                        "upstream media playlist unparseable",
                    );
                }
            };
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/vnd.apple.mpegurl"),
        );
        headers.insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store"),
        );
        return (StatusCode::OK, headers, rewritten).into_response();
    }

    // Raw segment: stream bytes through. Pass any Range header from the
    // player so the upstream's seekable mp4 keeps working.
    let mut req = state.client.get(upstream_url.as_str()).header(
        reqwest::header::REFERER,
        HeaderValue::from_str(&sess.referer)
            .unwrap_or_else(|_| HeaderValue::from_static("https://allmanga.to")),
    );
    if let Some(range) = headers_in.get("range") {
        if let Ok(rstr) = range.to_str() {
            req = req.header("Range", rstr);
        }
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream fetch failed"),
    };
    let status = resp.status();
    let headers = clone_passthrough_headers(resp.headers());
    let stream = resp.bytes_stream();
    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK),
        headers,
        Body::from_stream(stream),
    )
        .into_response()
}

async fn handle_sub(
    State(state): State<Arc<ProxyState>>,
    Path(session_str): Path<String>,
) -> Response {
    let session = match SessionId::parse(&session_str) {
        Ok(s) => s,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid session id"),
    };
    let Some(sess) = state.sessions.get(&session) else {
        return error_response(StatusCode::NOT_FOUND, "session not found");
    };
    let Some(sub_url) = sess.subtitle_url.clone() else {
        return error_response(StatusCode::NOT_FOUND, "no subtitle for this session");
    };

    let body = match upstream::fetch_text(&state.client, &sub_url, &sess.referer).await {
        Ok((b, _ct)) => b,
        Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream subtitle fetch"),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("text/vtt; charset=utf-8"),
    );
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("public, max-age=3600"),
    );
    (StatusCode::OK, headers, body).into_response()
}

fn decode_seg_url(b64: &str) -> crate::Result<Url> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|_| AniError::InvalidToken)?;
    let s = std::str::from_utf8(&bytes).map_err(|_| AniError::InvalidToken)?;
    Url::parse(s).map_err(|_| AniError::InvalidToken)
}

fn error_response(status: StatusCode, body: &'static str) -> Response {
    (status, body).into_response()
}

fn clone_passthrough_headers(src: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for h in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "etag",
        "last-modified",
    ] {
        if let Some(v) = src.get(h) {
            if let Ok(name) = HeaderName::from_bytes(h.as_bytes()) {
                if let Ok(value) = HeaderValue::from_bytes(v.as_bytes()) {
                    out.insert(name, value);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_loopback_kernel_assigns_port() {
        let (addr, listener) = bind_loopback(0).await.expect("bind");
        assert!(addr.ip().is_loopback());
        assert!(addr.port() > 0);
        // Listener actually accepts connections.
        let connect = tokio::net::TcpStream::connect(addr);
        let accept = listener.accept();
        let (a, b) = tokio::join!(connect, accept);
        a.expect("connect");
        b.expect("accept");
    }

    #[test]
    fn decode_seg_url_round_trip() {
        let u = "https://hianime.example/path/seg-001.ts";
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(u.as_bytes());
        let parsed = decode_seg_url(&enc).expect("decodes");
        assert_eq!(parsed.as_str(), u);
    }

    #[test]
    fn decode_seg_url_rejects_invalid_base64() {
        assert!(decode_seg_url("!!!").is_err());
    }

    #[test]
    fn decode_seg_url_rejects_non_url_payload() {
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"not a url");
        assert!(decode_seg_url(&enc).is_err());
    }
}
