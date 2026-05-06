//! Outbound `reqwest` client for the streaming proxy.
//!
//! Separate from the metadata client (Kitsu/AniList) so connection-pool
//! and timeout policy can differ — segments are large and latency-sensitive,
//! metadata calls are small and cacheable.
//!
//! The proxy never trusts the frontend's view of upstream URLs; the
//! [`StreamSession`](crate::proxy::token::StreamSession) it pulls from
//! the [`SessionTable`](crate::proxy::token::SessionTable) is the source
//! of truth for both the URL and the `Referer:` header.

use std::time::Duration;

use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, REFERER, USER_AGENT};
use url::Url;

use crate::error::{AniError, Result};

/// User-Agent used by every upstream fetch. Matches what `ani-cli`
/// presents so allanime CDNs see consistent traffic for one user.
pub const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0";

/// Build the proxy's outbound HTTP client with the right defaults.
///
/// # Errors
/// Returns [`AniError::Network`] if the underlying TLS stack cannot be
/// initialized (extremely rare).
pub fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(60))
        .timeout(Duration::from_secs(120))
        .gzip(true)
        .build()
        .map_err(|_| AniError::Network)
}

/// Fetch a manifest (HTTP body) from upstream with the right `Referer:`.
/// Used for master.m3u8 + media .m3u8 + .vtt.
///
/// Returns the raw bytes plus the response's `Content-Type` so the proxy
/// can echo it back to the player.
///
/// # Errors
/// - [`AniError::Network`] for connection or DNS failures
/// - [`AniError::Upstream`] when the response status is not 2xx
pub async fn fetch_text(
    client: &reqwest::Client,
    url: &Url,
    referer: &str,
) -> Result<(Bytes, Option<String>)> {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, v);
    }
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));

    let resp = client
        .get(url.as_str())
        .headers(headers)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let content_type = resp
        .headers()
        .get(HeaderName::from_static("content-type"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    Ok((bytes, content_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_succeeds() {
        let _c = build_client().expect("client builds");
    }

    #[tokio::test]
    async fn fetch_text_passes_referer_to_upstream() {
        // The Mock matcher requires the inbound Referer to match before
        // it will respond — so a successful 200 response *is* the proof
        // that the right Referer was sent. wiremock's body setter
        // overrides our explicit content-type, so we don't assert on it.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/master.m3u8"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/master.m3u8", server.uri())).unwrap();
        let (body, _ct) = fetch_text(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(&body[..], b"#EXTM3U\n");
    }

    #[tokio::test]
    async fn fetch_text_with_wrong_referer_yields_upstream_error() {
        // If the test sends a different Referer, wiremock's matcher fails
        // and the default response is 404 — proving that the Referer is
        // actually checked against the matcher.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/anything", server.uri())).unwrap();
        let err = fetch_text(&client, &url, "https://wrong.example")
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 404 }));
    }

    #[tokio::test]
    async fn fetch_text_returns_upstream_status_on_4xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/x", server.uri())).unwrap();
        let err = fetch_text(&client, &url, "https://allmanga.to")
            .await
            .unwrap_err();
        match err {
            AniError::Upstream { status } => assert_eq!(status, 403),
            other => panic!("expected Upstream {{status:403}}, got {other:?}"),
        }
    }
}
