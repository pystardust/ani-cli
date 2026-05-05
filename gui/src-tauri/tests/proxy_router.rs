//! End-to-end integration test for the streaming proxy router.
//!
//! Wires together:
//! - axum router from `ani_gui::proxy::build_router`
//! - a real localhost TCP listener bound by `bind_loopback`
//! - wiremock for upstream (manifests + segments)
//!
//! Exercises the full happy path:
//!
//! 1. The frontend would normally fetch `/s/<sid>/master.m3u8` — the proxy
//!    fetches the upstream master, rewrites variant URIs, returns the
//!    rewritten manifest.
//! 2. hls.js then fetches one of the rewritten variant URLs (`/s/<sid>/seg`),
//!    the proxy verifies the HMAC token, fetches upstream, and
//!    rewrites the media playlist.
//! 3. hls.js fetches a segment URL — proxy streams bytes through.
//!
//! Plus negative paths: invalid token rejection, expired session, missing
//! session.

use ani_gui::proxy::{
    bind_loopback, build_router, sign_segment, AppSecret, ProxyOrigin, ProxyState, SessionId,
    SessionTable, StreamSession,
};
use base64::Engine;
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Spin up wiremock + the proxy router; return everything the test needs
/// to make HTTP calls into both.
struct Harness {
    mock: MockServer,
    proxy_base: String,
    sessions: SessionTable,
    secret: AppSecret,
}

async fn start_harness() -> Harness {
    let mock = MockServer::start().await;
    let sessions = SessionTable::new();
    let secret = AppSecret::random();
    let client = ani_gui::proxy::upstream::build_client().expect("client");

    // First, bind to get the actual port for ProxyOrigin.
    let (addr, listener) = bind_loopback(0).await.expect("bind");
    let origin = ProxyOrigin::new("127.0.0.1", addr.port());

    let state = ProxyState {
        sessions: sessions.clone(),
        secret: secret.clone(),
        client,
        origin: origin.clone(),
    };

    let router = build_router(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Harness {
        mock,
        proxy_base: format!("http://127.0.0.1:{}", addr.port()),
        sessions,
        secret,
    }
}

fn make_session(h: &Harness, master_path: &str) -> SessionId {
    let upstream_url = Url::parse(&format!("{}{}", h.mock.uri(), master_path)).unwrap();
    let sess = StreamSession::new(upstream_url, "https://allmanga.to", None);
    let id = sess.id;
    h.sessions.insert(sess);
    id
}

#[tokio::test]
async fn healthz_returns_200_ok() {
    let h = start_harness().await;
    let body = reqwest::get(format!("{}/healthz", h.proxy_base))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn master_m3u8_is_fetched_rewritten_and_returned() {
    let h = start_harness().await;

    // Upstream serves a master playlist with one absolute variant URI.
    Mock::given(method("GET"))
        .and(path("/master.m3u8"))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n\
             #EXT-X-VERSION:3\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1920x1080\n\
             1080/index.m3u8\n",
        ))
        .mount(&h.mock)
        .await;

    let session = make_session(&h, "/master.m3u8");

    let url = format!("{}/s/{}/master.m3u8", h.proxy_base, session.as_string());
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    assert_eq!(ct.as_deref(), Some("application/vnd.apple.mpegurl"));
    let body = resp.text().await.unwrap();

    // The original relative URI is gone; a proxy URL with token replaces it.
    assert!(!body.contains("\n1080/index.m3u8\n"));
    assert!(body.contains(&format!(
        "{}/s/{}/seg?u=",
        h.proxy_base,
        session.as_string()
    )));
    assert!(body.contains("&t="));
}

#[tokio::test]
async fn segment_route_streams_bytes_after_token_verify() {
    let h = start_harness().await;

    let segment_path = "/v/1080/seg-001.ts";
    Mock::given(method("GET"))
        .and(path(segment_path))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(b"\x00\x01\x02segment-bytes".to_vec()),
        )
        .mount(&h.mock)
        .await;

    let session = make_session(&h, "/master.m3u8");
    let upstream_seg = format!("{}{}", h.mock.uri(), segment_path);
    let token = sign_segment(&h.secret, session, &upstream_seg);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(upstream_seg.as_bytes());
    let url = format!(
        "{}/s/{}/seg?u={}&t={}",
        h.proxy_base,
        session.as_string(),
        encoded,
        token
    );

    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(&body[..], b"\x00\x01\x02segment-bytes");
}

#[tokio::test]
async fn segment_route_rejects_invalid_token() {
    let h = start_harness().await;
    let session = make_session(&h, "/master.m3u8");
    let upstream_seg = format!("{}/v/seg.ts", h.mock.uri());
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(upstream_seg.as_bytes());
    let url = format!(
        "{}/s/{}/seg?u={}&t={}",
        h.proxy_base,
        session.as_string(),
        encoded,
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );

    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn unknown_session_returns_404() {
    let h = start_harness().await;
    let url = format!(
        "{}/s/{}/master.m3u8",
        h.proxy_base,
        SessionId::new().as_string()
    );
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn invalid_session_string_returns_400() {
    let h = start_harness().await;
    let url = format!("{}/s/not-a-uuid/master.m3u8", h.proxy_base);
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn segment_route_rewrites_inner_m3u8_too() {
    let h = start_harness().await;

    // Upstream master.
    Mock::given(method("GET"))
        .and(path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1920x1080\n\
             1080/index.m3u8\n",
        ))
        .mount(&h.mock)
        .await;
    // Upstream media playlist.
    Mock::given(method("GET"))
        .and(path("/1080/index.m3u8"))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n\
             #EXT-X-VERSION:3\n\
             #EXT-X-TARGETDURATION:6\n\
             #EXT-X-MEDIA-SEQUENCE:0\n\
             #EXTINF:6.0,\n\
             seg0.ts\n\
             #EXTINF:6.0,\n\
             seg1.ts\n\
             #EXT-X-ENDLIST\n",
        ))
        .mount(&h.mock)
        .await;

    let session = make_session(&h, "/master.m3u8");

    // Hit the master URL.
    let master_url = format!("{}/s/{}/master.m3u8", h.proxy_base, session.as_string());
    let master_body = reqwest::get(&master_url)
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // Extract one of the rewritten URLs from the master body, then GET it.
    let inner_url = master_body
        .lines()
        .find(|l| l.starts_with(&format!("{}/s/", h.proxy_base)))
        .expect("master contains a rewritten variant URL")
        .to_string();
    let media_resp = reqwest::get(&inner_url).await.unwrap();
    assert_eq!(media_resp.status(), 200);
    let media_body = media_resp.text().await.unwrap();
    // The inner playlist's relative segment URIs (seg0.ts, seg1.ts) are
    // rewritten too.
    assert!(!media_body.contains("\nseg0.ts\n"));
    assert!(!media_body.contains("\nseg1.ts\n"));
    assert!(media_body.matches("/seg?u=").count() >= 2);
}
