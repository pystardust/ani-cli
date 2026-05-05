//! HLS manifest rewriting.
//!
//! When the embedded `<video>` + hls.js player asks for a master playlist,
//! the streaming proxy fetches it from the upstream CDN, parses it, and
//! rewrites every variant + segment URI to flow back through itself with
//! HMAC-signed tokens. That way:
//!
//! - Browser fetches see only `127.0.0.1:<port>` URLs (no CORS pain).
//! - The upstream `Referer:` requirement is enforced server-side per
//!   request (browsers can't reliably set arbitrary `Referer:` headers).
//! - Tampering with segment URLs is detectable (HMAC mismatch).
//!
//! ## Functions
//!
//! - [`rewrite_master`] — parse a master playlist, rewrite each
//!   `EXT-X-STREAM-INF` and `EXT-X-MEDIA` URI, return the new manifest.
//! - [`rewrite_media`] — parse a media playlist, rewrite each segment
//!   URI, key URI, and init-segment URI.
//! - [`rewrite_uri`] (private) — resolve a relative URI against a base,
//!   then build a proxy URL with HMAC token.
//!
//! All functions are pure (no I/O). Property tests target idempotency.

use base64::Engine;
use url::Url;

use crate::error::{AniError, Result};
use crate::proxy::token::{sign_segment, AppSecret, SessionId};

/// How the proxy should render rewritten URIs back into the manifest.
/// Path style: `/s/<session>/seg?u=<base64-url-encoded-original>&t=<hmac>`
#[derive(Debug, Clone)]
pub struct ProxyOrigin {
    /// e.g. `http://127.0.0.1:42337` — no trailing slash.
    pub base: String,
}

impl ProxyOrigin {
    /// Build from host and port.
    #[must_use]
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            base: format!("http://{host}:{port}"),
        }
    }

    /// Render the segment URL the player will fetch.
    #[must_use]
    pub fn segment_url(&self, session: SessionId, original: &str, token: &str) -> String {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(original.as_bytes());
        format!(
            "{}/s/{}/seg?u={}&t={}",
            self.base,
            session.as_string(),
            encoded,
            token
        )
    }
}

/// Rewrite a master playlist. Returns the new manifest as a string.
///
/// `master_url` is the upstream URL the manifest came from; it's used to
/// resolve relative URIs.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the input isn't a valid HLS master
/// playlist.
pub fn rewrite_master(
    body: &[u8],
    master_url: &Url,
    origin: &ProxyOrigin,
    session: SessionId,
    secret: &AppSecret,
) -> Result<String> {
    let parsed = m3u8_rs::parse_master_playlist_res(body).map_err(|e| AniError::ParseFailed {
        detail: format!("master parse: {e}"),
    })?;
    let mut out = parsed;

    for v in &mut out.variants {
        let resolved = resolve(master_url, &v.uri)?;
        v.uri = build_proxy_uri(&resolved, origin, session, secret);
    }
    for a in &mut out.alternatives {
        if let Some(uri) = a.uri.as_mut() {
            let resolved = resolve(master_url, uri)?;
            *uri = build_proxy_uri(&resolved, origin, session, secret);
        }
    }

    let mut buf = Vec::with_capacity(body.len());
    out.write_to(&mut buf).map_err(|e| AniError::ParseFailed {
        detail: format!("master serialize: {e}"),
    })?;
    String::from_utf8(buf).map_err(|e| AniError::ParseFailed {
        detail: format!("master utf8: {e}"),
    })
}

/// Rewrite a media (variant) playlist. Returns the new manifest as a string.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the input isn't a valid HLS media
/// playlist.
pub fn rewrite_media(
    body: &[u8],
    media_url: &Url,
    origin: &ProxyOrigin,
    session: SessionId,
    secret: &AppSecret,
) -> Result<String> {
    let parsed = m3u8_rs::parse_media_playlist_res(body).map_err(|e| AniError::ParseFailed {
        detail: format!("media parse: {e}"),
    })?;
    let mut out = parsed;

    for seg in &mut out.segments {
        let resolved = resolve(media_url, &seg.uri)?;
        seg.uri = build_proxy_uri(&resolved, origin, session, secret);
        if let Some(map) = seg.map.as_mut() {
            let r = resolve(media_url, &map.uri)?;
            map.uri = build_proxy_uri(&r, origin, session, secret);
        }
        if let Some(k) = seg.key.as_mut() {
            if let Some(uri) = k.uri.as_mut() {
                let r = resolve(media_url, uri)?;
                *uri = build_proxy_uri(&r, origin, session, secret);
            }
        }
    }

    let mut buf = Vec::with_capacity(body.len());
    out.write_to(&mut buf).map_err(|e| AniError::ParseFailed {
        detail: format!("media serialize: {e}"),
    })?;
    String::from_utf8(buf).map_err(|e| AniError::ParseFailed {
        detail: format!("media utf8: {e}"),
    })
}

/// Resolve a URI string (absolute or relative) against a base URL.
fn resolve(base: &Url, uri: &str) -> Result<Url> {
    base.join(uri).map_err(|e| AniError::ParseFailed {
        detail: format!("resolve {uri:?} against {base}: {e}"),
    })
}

/// If the URI already points at our proxy origin, leave it alone
/// (idempotent rewrite). Otherwise build a fresh proxy URL with token.
fn build_proxy_uri(
    upstream: &Url,
    origin: &ProxyOrigin,
    session: SessionId,
    secret: &AppSecret,
) -> String {
    let upstream_str = upstream.as_str();
    if upstream_str.starts_with(&origin.base) {
        return upstream_str.to_string();
    }
    let tok = sign_segment(secret, session, upstream_str);
    origin.segment_url(session, upstream_str, &tok)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_origin() -> ProxyOrigin {
        ProxyOrigin::new("127.0.0.1", 42_337)
    }

    fn make_session() -> SessionId {
        SessionId::new()
    }

    #[test]
    fn rewrite_master_with_relative_variants() {
        let body = b"#EXTM3U\n\
                     #EXT-X-VERSION:3\n\
                     #EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1920x1080\n\
                     1080/index.m3u8\n\
                     #EXT-X-STREAM-INF:BANDWIDTH=600000,RESOLUTION=1280x720\n\
                     720/index.m3u8\n";
        let master_url = Url::parse("https://hianime.example/abc/master.m3u8").unwrap();
        let session = make_session();
        let secret = AppSecret::random();
        let origin = make_origin();

        let out = rewrite_master(body, &master_url, &origin, session, &secret).expect("rewrite ok");

        // Original relative URIs gone; proxy URLs present with token.
        assert!(!out.contains("1080/index.m3u8\n"));
        assert!(!out.contains("720/index.m3u8\n"));
        assert!(out.contains("http://127.0.0.1:42337/s/"));
        assert!(out.contains("/seg?u="));
        assert!(out.contains("&t="));
        // Stream-info attributes preserved.
        assert!(out.contains("BANDWIDTH=1000000"));
        assert!(out.contains("RESOLUTION=1920x1080"));
    }

    #[test]
    fn rewrite_master_is_idempotent() {
        let body = b"#EXTM3U\n\
                     #EXT-X-VERSION:3\n\
                     #EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1920x1080\n\
                     https://upstream.example/v/1080.m3u8\n";
        let master_url = Url::parse("https://upstream.example/master.m3u8").unwrap();
        let session = make_session();
        let secret = AppSecret::random();
        let origin = make_origin();

        let first = rewrite_master(body, &master_url, &origin, session, &secret).unwrap();
        // Use the rewritten output's "master URL" for the second pass too —
        // it would still resolve correctly because the URIs are absolute.
        let second =
            rewrite_master(first.as_bytes(), &master_url, &origin, session, &secret).unwrap();
        assert_eq!(first, second, "idempotency: second rewrite is a no-op");
    }

    #[test]
    fn rewrite_media_segments_and_init() {
        let body = b"#EXTM3U\n\
                     #EXT-X-VERSION:7\n\
                     #EXT-X-TARGETDURATION:6\n\
                     #EXT-X-MEDIA-SEQUENCE:0\n\
                     #EXT-X-MAP:URI=\"init.mp4\"\n\
                     #EXTINF:6.0,\n\
                     seg0.m4s\n\
                     #EXTINF:6.0,\n\
                     seg1.m4s\n\
                     #EXT-X-ENDLIST\n";
        let media_url = Url::parse("https://upstream.example/v/1080/index.m3u8").unwrap();
        let session = make_session();
        let secret = AppSecret::random();
        let origin = make_origin();

        let out = rewrite_media(body, &media_url, &origin, session, &secret).unwrap();
        // Segment URIs gone, proxy URIs present.
        assert!(!out.contains("\nseg0.m4s\n"));
        assert!(!out.contains("\nseg1.m4s\n"));
        // Init segment also rewritten (in EXT-X-MAP).
        assert!(!out.contains("URI=\"init.mp4\""));
        // Two segments + one map = three "/seg?u=" occurrences.
        let count = out.matches("/seg?u=").count();
        assert_eq!(
            count, 3,
            "two segments + one init = three rewrites; got {count}"
        );
    }

    #[test]
    fn rewrite_uri_skips_already_proxied() {
        let session = make_session();
        let secret = AppSecret::random();
        let origin = make_origin();
        let already = "http://127.0.0.1:42337/s/abc/seg?u=xxx&t=yyy";
        let url = Url::parse(already).unwrap();
        let s = build_proxy_uri(&url, &origin, session, &secret);
        assert_eq!(s, already, "URIs already on the proxy origin pass through");
    }

    #[test]
    fn resolve_relative_against_base() {
        let base = Url::parse("https://example.com/a/b/master.m3u8").unwrap();
        let r = resolve(&base, "1080/index.m3u8").unwrap();
        assert_eq!(r.as_str(), "https://example.com/a/b/1080/index.m3u8");
        let abs = resolve(&base, "https://other.example/x.m3u8").unwrap();
        assert_eq!(abs.as_str(), "https://other.example/x.m3u8");
    }
}
