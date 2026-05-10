//! `create_session` command — the frontend hands the backend a resolved
//! upstream HLS URL (with referer + optional subtitle) and gets back the
//! proxy URL hls.js / `<video>` should fetch.
//!
//! In M1.5 the upstream URL comes from a manual paste field; in M2+ it'll
//! come from the scraper output. The IPC contract is the same either way.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::app::AppState;
use crate::error::{AniError, Result};
use crate::proxy::{MediaKind, StreamSession};

/// Frontend → backend payload. All URLs are strings on the wire.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionArgs {
    /// Upstream master playlist (or single media playlist) URL.
    pub upstream_url: String,
    /// `Referer:` header the upstream CDN expects (empty string if none).
    pub referer: String,
    /// Optional WebVTT subtitle URL.
    pub subtitle_url: Option<String>,
}

/// What the frontend gets back: a session id, the proxy URL the
/// `<video>` element / hls.js feeds directly, and the kind so the
/// renderer knows which player to mount.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreateSessionResponse {
    /// Stringified UUID — used by the frontend if it needs to reference the
    /// session later (cancel, refresh).
    pub session_id: String,
    /// Full proxy URL the player should fetch. For HLS sessions this
    /// points at `…/s/<uuid>/master.m3u8`; for MP4 sessions at
    /// `…/s/<uuid>/file.mp4`. The frontend never has to compose the
    /// path itself.
    pub media_url: String,
    /// What kind of media the URL serves — drives hls.js vs `<video src>`
    /// on the renderer. Wire form is "hls" / "mp4" (lowercase).
    pub media_kind: MediaKind,
    /// Full proxy URL for the subtitle, when present.
    pub subtitle_url: Option<String>,
    /// `true` when the play resolution came from the long-term cache
    /// (no fresh ani-cli spawn). The renderer uses this to decide
    /// whether to silently evict + retry on a player error: a cache
    /// hit can be evicted and re-resolved, while a fresh-fetch failure
    /// already exhausted the resolve path and the user should see a
    /// real error. Defaults to false on construction; the play layer
    /// flips it when serving cached.
    #[serde(default)]
    pub cache_hit: bool,
}

/// Validate the inputs and register a new [`StreamSession`] in
/// [`AppState::sessions`]. Infers the [`MediaKind`] from the upstream
/// URL extension; callers with stronger signal (e.g. a HEAD response)
/// should use [`create_session_with_kind`] instead.
///
/// # Errors
/// - [`AniError::ParseFailed`] if `upstream_url` or `subtitle_url` is not a
///   parseable URL or uses a scheme other than `http`/`https`.
pub fn create_session(state: &AppState, args: &CreateSessionArgs) -> Result<CreateSessionResponse> {
    let upstream = parse_http_url(&args.upstream_url, "upstream_url")?;
    let kind = MediaKind::from_url(&upstream).unwrap_or(MediaKind::Hls);
    create_session_inner(state, args, upstream, kind)
}

/// Like [`create_session`], but takes an explicit [`MediaKind`] decided
/// by the caller (typically via a HEAD round-trip when the URL
/// extension is ambiguous).
///
/// # Errors
/// Same as [`create_session`].
pub fn create_session_with_kind(
    state: &AppState,
    args: &CreateSessionArgs,
    kind: MediaKind,
) -> Result<CreateSessionResponse> {
    let upstream = parse_http_url(&args.upstream_url, "upstream_url")?;
    create_session_inner(state, args, upstream, kind)
}

fn create_session_inner(
    state: &AppState,
    args: &CreateSessionArgs,
    upstream: Url,
    media_kind: MediaKind,
) -> Result<CreateSessionResponse> {
    let subtitle = match args.subtitle_url.as_deref() {
        None | Some("") => None,
        Some(s) => Some(parse_http_url(s, "subtitle_url")?),
    };

    let session =
        StreamSession::new_with_kind(upstream, media_kind, args.referer.clone(), subtitle.clone());
    let id = session.id;
    state.sessions.insert(session);

    let session_str = id.as_string();
    // The path segment matches the media kind — the proxy router has
    // a separate handler for each, so picking the wrong one would 415.
    let path = match media_kind {
        MediaKind::Hls => "master.m3u8",
        MediaKind::Mp4 => "file.mp4",
    };
    let media_url = format!("{}/s/{}/{}", state.proxy_origin.base, session_str, path);
    let subtitle_url =
        subtitle.map(|_| format!("{}/s/{}/sub.vtt", state.proxy_origin.base, session_str));

    Ok(CreateSessionResponse {
        session_id: session_str,
        media_url,
        media_kind,
        subtitle_url,
        cache_hit: false, // play layer flips when serving cached
    })
}

fn parse_http_url(s: &str, field: &str) -> Result<Url> {
    let u = Url::parse(s).map_err(|e| AniError::ParseFailed {
        detail: format!("{field}: {e}"),
    })?;
    match u.scheme() {
        "http" | "https" => Ok(u),
        other => Err(AniError::ParseFailed {
            detail: format!("{field}: scheme {other:?} not allowed"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    fn make_state(port: u16) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", port),
            ani_cli_path: PathBuf::from("/x"),
            history_path: PathBuf::from("/y/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(1)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
        }
    }

    #[test]
    fn create_session_returns_proxy_url_built_from_origin_and_session_id() {
        let state = make_state(40_000);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: "https://allmanga.to".into(),
                subtitle_url: None,
            },
        )
        .expect("ok");

        assert!(
            resp.media_url.starts_with("http://127.0.0.1:40000/s/"),
            "master url uses configured proxy origin: {}",
            resp.media_url
        );
        assert!(
            resp.media_url.ends_with("/master.m3u8"),
            "master url ends with master.m3u8: {}",
            resp.media_url
        );
        assert!(
            resp.media_url.contains(&resp.session_id),
            "master url embeds the returned session id"
        );
        assert_eq!(resp.subtitle_url, None);
    }

    #[test]
    fn create_session_inserts_into_session_table_with_referer_and_upstream() {
        let state = make_state(40_001);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: "https://allmanga.to".into(),
                subtitle_url: None,
            },
        )
        .unwrap();

        let id = crate::proxy::SessionId::parse(&resp.session_id).unwrap();
        let stored = state.sessions.get(&id).expect("session is in table");
        assert_eq!(
            stored.upstream_url.as_str(),
            "https://cdn.example/master.m3u8"
        );
        assert_eq!(stored.referer, "https://allmanga.to");
        assert!(stored.subtitle_url.is_none());
    }

    #[test]
    fn create_session_with_subtitle_returns_proxy_subtitle_url() {
        let state = make_state(40_002);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: "https://allmanga.to".into(),
                subtitle_url: Some("https://cdn.example/captions.vtt".into()),
            },
        )
        .unwrap();
        let sub = resp.subtitle_url.expect("subtitle url present");
        assert!(sub.starts_with("http://127.0.0.1:40002/s/"));
        assert!(sub.ends_with("/sub.vtt"));
        assert!(sub.contains(&resp.session_id));
    }

    #[test]
    fn empty_subtitle_string_is_treated_as_none() {
        let state = make_state(40_003);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: String::new(),
                subtitle_url: Some(String::new()),
            },
        )
        .unwrap();
        assert_eq!(resp.subtitle_url, None);
    }

    #[test]
    fn invalid_upstream_url_returns_parse_failed() {
        let state = make_state(40_004);
        let r = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "not a url".into(),
                referer: String::new(),
                subtitle_url: None,
            },
        );
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
        assert_eq!(state.sessions.len(), 0, "no session leaked on error");
    }

    #[test]
    fn non_http_upstream_scheme_is_rejected() {
        let state = make_state(40_005);
        let r = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "file:///etc/passwd".into(),
                referer: String::new(),
                subtitle_url: None,
            },
        );
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
        assert_eq!(state.sessions.len(), 0);
    }

    #[test]
    fn invalid_subtitle_url_returns_parse_failed_and_does_not_leak_session() {
        let state = make_state(40_006);
        let r = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: String::new(),
                subtitle_url: Some("ftp://x/y.vtt".into()),
            },
        );
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
        assert_eq!(state.sessions.len(), 0, "validation runs before insert");
    }

    #[test]
    fn http_scheme_is_accepted() {
        let state = make_state(40_007);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "http://insecure.example/master.m3u8".into(),
                referer: String::new(),
                subtitle_url: None,
            },
        )
        .expect("plain http allowed");
        assert_eq!(state.sessions.len(), 1);
        assert!(resp.media_url.contains(&resp.session_id));
    }

    #[test]
    fn response_serializes_with_camel_case_session_fields() {
        // Sanity: the JSON shape is what the frontend wrapper expects.
        let r = CreateSessionResponse {
            session_id: "abc".into(),
            media_url: "http://x/m".into(),
            media_kind: crate::proxy::MediaKind::Hls,
            subtitle_url: None,
            cache_hit: false,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"session_id\":\"abc\""));
        assert!(s.contains("\"media_url\":\"http://x/m\""));
        assert!(s.contains("\"media_kind\":\"hls\""));
        assert!(s.contains("\"subtitle_url\":null"));
    }

    /// HLS upstreams (.m3u8) get a media_url that points at the
    /// /master.m3u8 proxy endpoint and a media_kind of "hls"; MP4
    /// upstreams (.mp4) get /file.mp4 and "mp4". The renderer uses
    /// media_kind to pick hls.js vs `<video src>`, so the response
    /// must carry both signals atomically.
    #[test]
    fn create_session_with_mp4_upstream_returns_mp4_kind_and_file_mp4_url() {
        let state = make_state(40_010);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://video.example/1080/file.mp4".into(),
                referer: "https://allmanga.to".into(),
                subtitle_url: None,
            },
        )
        .expect("ok");
        assert_eq!(resp.media_kind, crate::proxy::MediaKind::Mp4);
        assert!(
            resp.media_url.ends_with("/file.mp4"),
            "media url ends with file.mp4 for MP4 sessions: {}",
            resp.media_url
        );
        assert!(resp.media_url.contains(&resp.session_id));
    }

    #[test]
    fn create_session_with_hls_upstream_returns_hls_kind_and_master_m3u8_url() {
        let state = make_state(40_011);
        let resp = create_session(
            &state,
            &CreateSessionArgs {
                upstream_url: "https://cdn.example/master.m3u8".into(),
                referer: "https://allmanga.to".into(),
                subtitle_url: None,
            },
        )
        .expect("ok");
        assert_eq!(resp.media_kind, crate::proxy::MediaKind::Hls);
        assert!(
            resp.media_url.ends_with("/master.m3u8"),
            "media url ends with master.m3u8 for HLS sessions: {}",
            resp.media_url
        );
    }
}
