//! HMAC-signed segment tokens + the in-memory stream-session table.
//!
//! ## Design
//!
//! When the backend resolves a stream URL (via [`crate::anicli::process::run_debug`])
//! it creates a [`StreamSession`] and stores it in a process-global table
//! keyed by [`SessionId`] (a UUID). The session holds the upstream master
//! URL, the `Referer:` header the CDN requires, and a TTL.
//!
//! The frontend's `<video>` + hls.js fetch goes to the proxy at
//! `http://127.0.0.1:<port>/s/<session>/master.m3u8`. For each segment the
//! proxy rewrites into the manifest, the URL carries an HMAC-signed token
//! `t = base64url(hmac(secret, session_id || segment_url))`. The proxy
//! verifies `t` before forwarding the segment fetch upstream.
//!
//! Sessions don't authenticate the user (everything is `127.0.0.1`); they
//! prevent a malicious local page from guessing a port and replaying
//! arbitrary upstream URLs through our proxy.
//!
//! ## What lives here
//!
//! - [`AppSecret`] — per-process HMAC key, generated on app startup.
//! - [`SessionId`], [`StreamSession`] — typed wrappers around the data.
//! - [`SessionTable`] — concurrent map; sessions garbage-collected on
//!   read against `expires_at`.
//! - [`sign_segment`] / [`verify_segment`] — pure HMAC helpers, easy to
//!   property-test.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::{AniError, Result};

type HmacSha256 = Hmac<Sha256>;

/// Default TTL for newly created sessions.
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(4 * 60 * 60);

/// Per-process HMAC key. 32 random bytes generated at app startup.
#[derive(Clone, Debug)]
pub struct AppSecret(Arc<[u8; 32]>);

impl AppSecret {
    /// Generate a fresh secret with a CSPRNG. Call once per process.
    #[must_use]
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(Arc::new(bytes))
    }

    /// Construct from a known seed (test only — never call in prod).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Arc::new(bytes))
    }

    fn key_bytes(&self) -> &[u8] {
        &self.0[..]
    }
}

/// Strongly typed UUID for stream sessions. Wraps [`Uuid`] for clarity at
/// API boundaries.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Generate a new random session id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Render as a 32-char hyphenated string (matches Uuid's default form).
    #[must_use]
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }

    /// Parse from a 32-char hyphenated string.
    ///
    /// # Errors
    /// Returns [`AniError::InvalidToken`] when the input isn't a valid UUID.
    pub fn parse(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| AniError::InvalidToken)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// One playback session. Created when the user clicks Play on an episode.
/// Held in [`SessionTable`] until it expires.
#[derive(Debug, Clone)]
pub struct StreamSession {
    /// Session identifier (used as the path segment in proxy URLs).
    pub id: SessionId,
    /// Upstream master playlist or mp4 URL we resolved via ani-cli.
    pub upstream_url: url::Url,
    /// `Referer:` header the upstream CDN requires.
    pub referer: String,
    /// Optional subtitle (`.vtt`) URL — also proxied with referer injection.
    pub subtitle_url: Option<url::Url>,
    /// Wall-clock expiry. After this point the session is GC'd on next read.
    pub expires_at: SystemTime,
}

impl StreamSession {
    /// Build a session with the default TTL.
    #[must_use]
    pub fn new(
        upstream_url: url::Url,
        referer: impl Into<String>,
        subtitle_url: Option<url::Url>,
    ) -> Self {
        Self {
            id: SessionId::new(),
            upstream_url,
            referer: referer.into(),
            subtitle_url,
            expires_at: SystemTime::now() + DEFAULT_SESSION_TTL,
        }
    }

    /// Has the session expired?
    #[must_use]
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }
}

/// Concurrent table of live stream sessions. Cheap to clone (`Arc` inside).
#[derive(Clone, Default)]
pub struct SessionTable {
    inner: Arc<DashMap<SessionId, Arc<StreamSession>>>,
}

impl SessionTable {
    /// Empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a session. Returns its id for the caller to embed in URLs.
    pub fn insert(&self, session: StreamSession) -> SessionId {
        let id = session.id;
        self.inner.insert(id, Arc::new(session));
        id
    }

    /// Fetch a session by id. Returns `None` for missing or expired entries
    /// (and removes expired entries as a side effect — lazy GC).
    #[must_use]
    pub fn get(&self, id: &SessionId) -> Option<Arc<StreamSession>> {
        let arc = self.inner.get(id)?.clone();
        if arc.is_expired() {
            self.inner.remove(id);
            return None;
        }
        Some(arc)
    }

    /// Remove a session unconditionally.
    pub fn remove(&self, id: &SessionId) {
        self.inner.remove(id);
    }

    /// Sweep all expired sessions. Intended for periodic background calls.
    /// Returns the number of entries evicted.
    pub fn sweep_expired(&self) -> usize {
        let mut to_remove = Vec::new();
        for entry in self.inner.iter() {
            if entry.value().is_expired() {
                to_remove.push(*entry.key());
            }
        }
        let n = to_remove.len();
        for id in to_remove {
            self.inner.remove(&id);
        }
        n
    }

    /// Number of sessions currently held (including any not yet GC'd).
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Sign a segment URL for a given session. Returns a base64url-encoded
/// 32-byte HMAC-SHA256 with no padding. The token is short enough to fit
/// comfortably as a query parameter in rewritten manifests.
#[must_use]
pub fn sign_segment(secret: &AppSecret, session: SessionId, segment_url: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.key_bytes()).expect("HMAC-SHA256 accepts any key length");
    mac.update(session.0.as_bytes());
    mac.update(b"|");
    mac.update(segment_url.as_bytes());
    let bytes = mac.finalize().into_bytes();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Verify a token against a (`session`, `segment_url`) pair. Constant-time
/// comparison via the HMAC crate's `verify_slice`.
///
/// # Errors
/// Returns [`AniError::InvalidToken`] when:
/// - the token is not valid base64url
/// - the decoded length is not 32 bytes
/// - the HMAC does not match
pub fn verify_segment(
    secret: &AppSecret,
    session: SessionId,
    segment_url: &str,
    token: &str,
) -> Result<()> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| AniError::InvalidToken)?;
    if bytes.len() != 32 {
        return Err(AniError::InvalidToken);
    }
    let mut mac =
        HmacSha256::new_from_slice(secret.key_bytes()).expect("HMAC-SHA256 accepts any key length");
    mac.update(session.0.as_bytes());
    mac.update(b"|");
    mac.update(segment_url.as_bytes());
    mac.verify_slice(&bytes).map_err(|_| AniError::InvalidToken)
}

/// Diagnostic: how long until a session's expiry, in seconds (0 if expired).
#[must_use]
pub fn seconds_until_expiry(session: &StreamSession) -> u64 {
    session
        .expires_at
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|exp| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|now| (exp, now))
        })
        .map(|(exp, now)| exp.saturating_sub(now).as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_secret() -> AppSecret {
        AppSecret::from_bytes([7u8; 32])
    }

    /// ani-cli's "Selected link:" can be either an HLS master playlist
    /// (`*.m3u8`) or a direct MP4 (`*.mp4` from wixmp/sharepoint/etc).
    /// The proxy needs to know which so it can either rewrite the
    /// manifest or stream the bytes through with `Range` support; the
    /// frontend needs to know which so it can pick hls.js vs a plain
    /// `<video src=...>`. URL-extension detection is the fast path.
    #[test]
    fn media_kind_from_url_detects_hls() {
        let url = url::Url::parse("https://example.com/abc/master.m3u8").unwrap();
        assert_eq!(MediaKind::from_url(&url), Some(MediaKind::Hls));
    }

    #[test]
    fn media_kind_from_url_detects_mp4() {
        let url =
            url::Url::parse("https://video.wixstatic.com/video/bd1bd7_xxxx/1080p/mp4/file.mp4")
                .unwrap();
        assert_eq!(MediaKind::from_url(&url), Some(MediaKind::Mp4));
    }

    #[test]
    fn media_kind_from_url_ignores_query_and_fragment() {
        let with_query =
            url::Url::parse("https://example.com/abc/master.m3u8?token=xyz&exp=42").unwrap();
        assert_eq!(MediaKind::from_url(&with_query), Some(MediaKind::Hls));
        let with_frag = url::Url::parse("https://example.com/file.mp4#t=0").unwrap();
        assert_eq!(MediaKind::from_url(&with_frag), Some(MediaKind::Mp4));
    }

    #[test]
    fn media_kind_from_url_returns_none_for_unknown_extension() {
        let url = url::Url::parse("https://example.com/embed/abc123").unwrap();
        assert_eq!(MediaKind::from_url(&url), None);
    }

    #[test]
    fn media_kind_from_url_is_case_insensitive() {
        let url = url::Url::parse("https://example.com/PLAYLIST.M3U8").unwrap();
        assert_eq!(MediaKind::from_url(&url), Some(MediaKind::Hls));
    }

    #[test]
    fn stream_session_carries_media_kind() {
        let url = url::Url::parse("https://video.example/file.mp4").unwrap();
        let sess = StreamSession::new(url, "ref", None);
        assert_eq!(sess.media_kind, MediaKind::Mp4);
    }

    #[test]
    fn signed_token_roundtrips() {
        let secret = fixed_secret();
        let sid = SessionId::new();
        let url = "https://example.com/segment-001.ts";
        let tok = sign_segment(&secret, sid, url);
        verify_segment(&secret, sid, url, &tok).expect("good token verifies");
    }

    #[test]
    fn token_with_wrong_session_rejected() {
        let secret = fixed_secret();
        let sid_a = SessionId::new();
        let sid_b = SessionId::new();
        let url = "https://example.com/segment.ts";
        let tok = sign_segment(&secret, sid_a, url);
        let r = verify_segment(&secret, sid_b, url, &tok);
        assert!(matches!(r, Err(AniError::InvalidToken)));
    }

    #[test]
    fn token_with_wrong_url_rejected() {
        let secret = fixed_secret();
        let sid = SessionId::new();
        let tok = sign_segment(&secret, sid, "https://example.com/a.ts");
        let r = verify_segment(&secret, sid, "https://example.com/b.ts", &tok);
        assert!(matches!(r, Err(AniError::InvalidToken)));
    }

    #[test]
    fn token_with_wrong_secret_rejected() {
        let secret_a = AppSecret::from_bytes([1u8; 32]);
        let secret_b = AppSecret::from_bytes([2u8; 32]);
        let sid = SessionId::new();
        let url = "https://example.com/seg.ts";
        let tok = sign_segment(&secret_a, sid, url);
        assert!(matches!(
            verify_segment(&secret_b, sid, url, &tok),
            Err(AniError::InvalidToken)
        ));
    }

    #[test]
    fn malformed_token_rejected() {
        let secret = fixed_secret();
        let sid = SessionId::new();
        // Not valid base64url.
        assert!(matches!(
            verify_segment(&secret, sid, "url", "!!!"),
            Err(AniError::InvalidToken)
        ));
        // Valid base64url but wrong length.
        let short = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"abc");
        assert!(matches!(
            verify_segment(&secret, sid, "url", &short),
            Err(AniError::InvalidToken)
        ));
    }

    #[test]
    fn session_table_round_trip() {
        let table = SessionTable::new();
        let sess = StreamSession::new(
            url::Url::parse("https://example.com/master.m3u8").unwrap(),
            "https://allmanga.to",
            None,
        );
        let id = sess.id;
        assert_eq!(table.len(), 0);
        let returned = table.insert(sess);
        assert_eq!(id, returned);
        assert_eq!(table.len(), 1);
        let pulled = table.get(&id).expect("present");
        assert_eq!(pulled.id, id);
        assert_eq!(pulled.referer, "https://allmanga.to");
        table.remove(&id);
        assert!(table.is_empty());
    }

    #[test]
    fn expired_session_is_evicted_on_get() {
        let table = SessionTable::new();
        let mut sess = StreamSession::new(
            url::Url::parse("https://example.com/master.m3u8").unwrap(),
            "ref",
            None,
        );
        sess.expires_at = SystemTime::now() - Duration::from_secs(1);
        let id = table.insert(sess);
        assert!(table.get(&id).is_none(), "expired sessions return None");
        assert!(table.is_empty(), "and are removed from the map");
    }

    #[test]
    fn sweep_evicts_only_expired() {
        let table = SessionTable::new();
        let live = StreamSession::new(
            url::Url::parse("https://a.example/m.m3u8").unwrap(),
            "ref",
            None,
        );
        let mut dead = StreamSession::new(
            url::Url::parse("https://b.example/m.m3u8").unwrap(),
            "ref",
            None,
        );
        dead.expires_at = SystemTime::now() - Duration::from_secs(1);
        let live_id = table.insert(live);
        let dead_id = table.insert(dead);
        assert_eq!(table.sweep_expired(), 1);
        assert!(table.get(&live_id).is_some());
        assert!(table.get(&dead_id).is_none());
    }

    // — Properties ────────────────────────────────────────────────────
    //
    // The HMAC sign/verify pair has two load-bearing invariants:
    //
    //   1. Roundtrip: a token produced by `sign_segment(s, sid, url)` must
    //      always verify against the same triple. If this ever breaks, the
    //      streaming proxy starts rejecting its own legitimate manifests.
    //
    //   2. Tamper rejection: flipping any bit of the token, the URL, the
    //      session id, or the secret must produce a verification failure.
    //      This is the actual security boundary — without it, an attacker
    //      with renderer-side code execution could synthesize tokens for
    //      arbitrary upstream URLs.
    //
    // Both are checked across randomly-generated inputs by `proptest`,
    // which shrinks failures to a minimal counter-example.
    use proptest::prelude::*;

    /// Plausible upstream URL strings — limited to https and a small
    /// alphabet so proptest doesn't burn cycles generating exotic UTF-8
    /// that the HMAC layer doesn't actually care about.
    fn url_strategy() -> impl Strategy<Value = String> {
        r"https://[a-zA-Z0-9._/-]{5,120}".prop_map(String::from)
    }

    proptest! {
        /// Sign-then-verify always succeeds for the same triple.
        #[test]
        fn sign_then_verify_roundtrips(
            secret_bytes in any::<[u8; 32]>(),
            session_uuid in any::<u128>(),
            segment_url in url_strategy(),
        ) {
            let secret = AppSecret::from_bytes(secret_bytes);
            let session = SessionId(uuid::Uuid::from_u128(session_uuid));
            let token = sign_segment(&secret, session, &segment_url);
            verify_segment(&secret, session, &segment_url, &token).expect("matching token verifies");
        }

        /// A different secret never validates a legitimate token.
        #[test]
        fn token_does_not_verify_under_different_secret(
            a in any::<[u8; 32]>(),
            b in any::<[u8; 32]>(),
            session_uuid in any::<u128>(),
            segment_url in url_strategy(),
        ) {
            prop_assume!(a != b);
            let session = SessionId(uuid::Uuid::from_u128(session_uuid));
            let token = sign_segment(&AppSecret::from_bytes(a), session, &segment_url);
            let res = verify_segment(&AppSecret::from_bytes(b), session, &segment_url, &token);
            prop_assert!(res.is_err());
        }

        /// Wrong URL never validates against a token issued for a different URL.
        #[test]
        fn token_does_not_verify_for_different_url(
            secret_bytes in any::<[u8; 32]>(),
            session_uuid in any::<u128>(),
            url_a in url_strategy(),
            url_b in url_strategy(),
        ) {
            prop_assume!(url_a != url_b);
            let secret = AppSecret::from_bytes(secret_bytes);
            let session = SessionId(uuid::Uuid::from_u128(session_uuid));
            let token = sign_segment(&secret, session, &url_a);
            let res = verify_segment(&secret, session, &url_b, &token);
            prop_assert!(res.is_err());
        }

        /// Wrong session id never validates against a token issued for a
        /// different session — this is what stops cross-session token
        /// reuse if a renderer ever leaks a legitimate token.
        #[test]
        fn token_does_not_verify_for_different_session(
            secret_bytes in any::<[u8; 32]>(),
            session_a in any::<u128>(),
            session_b in any::<u128>(),
            segment_url in url_strategy(),
        ) {
            prop_assume!(session_a != session_b);
            let secret = AppSecret::from_bytes(secret_bytes);
            let token = sign_segment(
                &secret,
                SessionId(uuid::Uuid::from_u128(session_a)),
                &segment_url,
            );
            let res = verify_segment(
                &secret,
                SessionId(uuid::Uuid::from_u128(session_b)),
                &segment_url,
                &token,
            );
            prop_assert!(res.is_err());
        }
    }
}
