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
    bind_loopback, build_router, sign_segment, AppSecret, MediaKind, ProxyOrigin, ProxyState,
    SessionId, SessionTable, StreamSession,
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

fn make_mp4_session(h: &Harness, mp4_path: &str) -> SessionId {
    let upstream_url = Url::parse(&format!("{}{}", h.mock.uri(), mp4_path)).unwrap();
    let sess =
        StreamSession::new_with_kind(upstream_url, MediaKind::Mp4, "https://allmanga.to", None);
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
async fn subtitle_route_proxies_vtt_with_text_vtt_content_type() {
    let h = start_harness().await;
    Mock::given(method("GET"))
        .and(path("/captions.vtt"))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n"),
        )
        .mount(&h.mock)
        .await;

    let upstream = Url::parse(&format!("{}/master.m3u8", h.mock.uri())).unwrap();
    let subtitle = Url::parse(&format!("{}/captions.vtt", h.mock.uri())).unwrap();
    let sess = StreamSession::new(upstream, "https://allmanga.to", Some(subtitle));
    let id = h.sessions.insert(sess);

    let url = format!("{}/s/{}/sub.vtt", h.proxy_base, id.as_string());
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("text/vtt"));
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("WEBVTT"));
    assert!(body.contains("Hello"));
}

#[tokio::test]
async fn subtitle_route_returns_404_when_session_has_no_subtitle() {
    let h = start_harness().await;
    let session = make_session(&h, "/master.m3u8");
    let url = format!("{}/s/{}/sub.vtt", h.proxy_base, session.as_string());
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 404);
}

/// Direct-MP4 upstreams (wixmp, sharepoint, fast4speed) need a different
/// proxy path than HLS — no manifest to rewrite, just byte-stream the
/// response through. The renderer feeds `/s/<id>/file.mp4` to a plain
/// `<video src=...>` instead of hls.js, so the proxy must:
///   - return 200 with `content-type: video/mp4` for full requests,
///   - forward `Range` headers and pass back 206 + `Content-Range`,
///   - propagate the upstream `Accept-Ranges: bytes` header so the
///     renderer surfaces a working scrub bar.
#[tokio::test]
async fn mp4_route_streams_full_body_with_video_mp4_content_type() {
    let h = start_harness().await;
    let body = b"\x00\x00\x00\x18ftyp...mp4-bytes-here".to_vec();
    Mock::given(method("GET"))
        .and(path("/file.mp4"))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "video/mp4")
                .insert_header("accept-ranges", "bytes")
                .insert_header("content-length", &body.len().to_string())
                .set_body_bytes(body.clone()),
        )
        .mount(&h.mock)
        .await;

    let session = make_mp4_session(&h, "/file.mp4");
    let url = format!("{}/s/{}/file.mp4", h.proxy_base, session.as_string());
    let resp = reqwest::get(&url).await.expect("proxy responds");
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap()),
        Some("video/mp4"),
        "proxy preserves upstream content-type"
    );
    assert_eq!(
        resp.headers()
            .get("accept-ranges")
            .map(|v| v.to_str().unwrap()),
        Some("bytes"),
        "Accept-Ranges propagates so the player shows a scrub bar",
    );
    let got = resp.bytes().await.unwrap().to_vec();
    assert_eq!(got, body);
}

#[tokio::test]
async fn mp4_route_forwards_range_header_and_returns_206() {
    let h = start_harness().await;
    // Mock specifically for the Range request — wiremock matches the
    // header and responds with the partial-content shape.
    let partial = b"FTYP-BYTES".to_vec();
    Mock::given(method("GET"))
        .and(path("/file.mp4"))
        .and(header("range", "bytes=0-9"))
        .and(header("referer", "https://allmanga.to"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("content-type", "video/mp4")
                .insert_header("content-range", "bytes 0-9/1024")
                .insert_header("accept-ranges", "bytes")
                .insert_header("content-length", "10")
                .set_body_bytes(partial.clone()),
        )
        .mount(&h.mock)
        .await;

    let session = make_mp4_session(&h, "/file.mp4");
    let url = format!("{}/s/{}/file.mp4", h.proxy_base, session.as_string());

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("range", "bytes=0-9")
        .send()
        .await
        .expect("proxy responds");
    assert_eq!(resp.status(), 206);
    assert_eq!(
        resp.headers()
            .get("content-range")
            .map(|v| v.to_str().unwrap()),
        Some("bytes 0-9/1024"),
        "Content-Range propagates back from upstream",
    );
    let got = resp.bytes().await.unwrap().to_vec();
    assert_eq!(got, partial);
}

/// Wrong-endpoint contract: hitting `/master.m3u8` on an MP4-kind session
/// or `/file.mp4` on an HLS-kind session should be a clean 415 — not a
/// hung manifest fetch (the existing bug).
#[tokio::test]
async fn master_m3u8_route_rejects_mp4_session_with_415() {
    let h = start_harness().await;
    let session = make_mp4_session(&h, "/file.mp4");
    let url = format!("{}/s/{}/master.m3u8", h.proxy_base, session.as_string());
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(
        resp.status(),
        415,
        "HLS endpoint refuses MP4 sessions instead of trying to rewrite",
    );
}

#[tokio::test]
async fn mp4_route_rejects_hls_session_with_415() {
    let h = start_harness().await;
    let session = make_session(&h, "/master.m3u8");
    let url = format!("{}/s/{}/file.mp4", h.proxy_base, session.as_string());
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), 415, "MP4 endpoint refuses HLS sessions",);
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
