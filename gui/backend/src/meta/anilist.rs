//! AniList GraphQL client.
//!
//! Used today only for the home page's Trending Now row — AniList's
//! `TRENDING_DESC` sort is genuinely week-fresh (it weights recent
//! activity surge), unlike Kitsu's `userCount` which is cumulative
//! across all time and lets evergreens like One Piece anchor the top
//! forever. The plan-doc rationale is in `requirements.md` §7 / D2.
//!
//! Read-only public queries don't require auth. AniList rate-limits
//! all clients to 30 requests/minute (per IP). With a 30-min cache
//! on the trending fetch, we use ~2 requests/hour — well under.
//!
//! Cross-references: each `Media` entry exposes `idMal`
//! (MyAnimeList id), which the home page bridges to a Kitsu id
//! through Kitsu's `mappings` endpoint to keep nav + the rest of the
//! app on Kitsu's id space.

use serde::Deserialize;

use crate::error::{AniError, Result};

const ANILIST_API: &str = "https://graphql.anilist.co";

/// One trending anime as AniList serves it. Fields chosen to match
/// what the home-page bridge consumes: `id_mal` for the Kitsu lookup,
/// `title.user_preferred` for fallback display when the bridge
/// fails, the rest available for richer rendering when we want it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListAnimeRef {
    /// AniList's own id.
    pub id: u32,
    /// MyAnimeList id. The bridge to Kitsu — Kitsu's `mappings`
    /// endpoint accepts `filter[externalSite]=myanimelist/anime` +
    /// `filter[externalId]=<id_mal>`. May be null on shows AniList
    /// indexes but MAL doesn't (rare).
    #[serde(rename = "idMal")]
    pub id_mal: Option<u32>,
    /// Title bag — same shape as Kitsu's `titles` map but with fixed
    /// keys. `user_preferred` is the field AniList renders by default
    /// in their own UI; safe display fallback.
    pub title: AniListTitle,
    /// Cover (poster) image bag. AniList serves three pre-rendered
    /// sizes plus an extracted dominant colour for theming.
    #[serde(rename = "coverImage")]
    pub cover_image: AniListCoverImage,
    /// Single banner URL (~21:5). May be null on shows that don't
    /// have a banner uploaded; the renderer falls back to the cover
    /// in that case.
    #[serde(rename = "bannerImage")]
    pub banner_image: Option<String>,
    /// AniList airing status — `"RELEASING"`, `"FINISHED"`,
    /// `"NOT_YET_RELEASED"`, `"CANCELLED"`, `"HIATUS"`.
    pub status: Option<String>,
    /// Total announced episode count. Null on shows without a
    /// confirmed total.
    pub episodes: Option<u32>,
    /// AniList's trending score for this entry — rough surrogate for
    /// "users who interacted in the last few days." Higher = hotter.
    pub trending: Option<u32>,
    /// Mean rating × 100 (0..=100). Optional because not every show
    /// has enough scores to compute one.
    #[serde(rename = "averageScore")]
    pub average_score: Option<u32>,
}

/// AniList exposes four well-known title forms. `user_preferred` is
/// the one the AniList UI itself defaults to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
    #[serde(rename = "userPreferred")]
    pub user_preferred: Option<String>,
}

/// AniList cover-image bag. Sizes from largest to smallest plus an
/// extracted dominant colour string (`"#1abbd6"` etc.) usable as a
/// theming accent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListCoverImage {
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub medium: Option<String>,
    /// `"#rrggbb"` — already in the format CSS expects.
    pub color: Option<String>,
}

/// GraphQL query string for the trending feed. Field set is the
/// minimum the bridge + frontend need; expand here when adding new
/// surfaces. `perPage` is bound at call time.
const TRENDING_GQL: &str = "query Trending($perPage: Int!) { \
    Page(perPage: $perPage) { \
        media(type: ANIME, sort: TRENDING_DESC) { \
            id idMal \
            title { romaji english native userPreferred } \
            coverImage { extraLarge large medium color } \
            bannerImage \
            status episodes trending averageScore \
        } \
    } \
}";

/// Lightweight by-MAL-id query — used by detail-page enrichment
/// when Kitsu's coverImage is null and we need a banner fallback.
/// Smaller projection than [`TRENDING_GQL`] since the caller only
/// needs the banner URL.
const BANNER_BY_MAL_GQL: &str = "query BannerByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { bannerImage } \
    }";

/// Fetch the AniList trending feed, top `limit` entries.
///
/// `base_override` mirrors the convention in `scraper::allanime` —
/// `None` in prod (hits the real GraphQL endpoint), `Some(uri)` in
/// tests pointing at wiremock.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the response shape is wrong.
pub async fn trending(
    client: &reqwest::Client,
    limit: u8,
    base_override: Option<&str>,
) -> Result<Vec<AniListAnimeRef>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": TRENDING_GQL,
        "variables": { "perPage": limit },
    });
    // Override the proxy client's Firefox-mimic UA with an app-style
    // identifier — AniList's Cloudflare layer blocks browser UAs
    // that lack a full browser fingerprint and returns 403. Curl
    // and any UA that doesn't claim to be a browser pass through.
    let resp = client
        .post(url)
        .header(
            "user-agent",
            "ani-gui/0.1 (https://github.com/pucci/ani-gui)",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    parse_trending(&bytes)
}

/// Look up the AniList banner URL for a show by its MAL id.
/// Returns `None` when AniList has no media for the supplied id, or
/// when AniList has the media but no banner uploaded. Used by the
/// detail-page enrichment chain (Kitsu null cover → MAL id → here).
///
/// # Errors
/// Same as [`trending`] — Network / Upstream / ParseFailed.
pub async fn banner_for_mal_id(
    client: &reqwest::Client,
    mal_id: u32,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": BANNER_BY_MAL_GQL,
        "variables": { "idMal": mal_id },
    });
    let resp = client
        .post(url)
        .header(
            "user-agent",
            "ani-gui/0.1 (https://github.com/pucci/ani-gui)",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    parse_banner_response(&bytes)
}

/// Pure parser for the by-MAL banner response.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ data: { Media: { bannerImage } } }` envelope.
pub fn parse_banner_response(body: &[u8]) -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(rename = "bannerImage")]
        banner_image: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist banner response: {e}"),
    })?;
    Ok(parsed.data.media.and_then(|m| m.banner_image))
}

/// Pure parser for the trending response body.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the JSON doesn't shape
/// into `{ data: { Page: { media: [...] } } }`.
pub fn parse_trending(body: &[u8]) -> Result<Vec<AniListAnimeRef>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Page")]
        page: Page,
    }
    #[derive(Deserialize)]
    struct Page {
        media: Vec<AniListAnimeRef>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist trending response: {e}"),
    })?;
    Ok(parsed.data.page.media)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real shape lifted from a live AniList trending probe. Three
    /// entries; covers the optional fields (idMal, episodes,
    /// bannerImage) hitting both Some and null variants so the
    /// derive's `Option` handling is exercised.
    fn fixture_body() -> &'static [u8] {
        // Regular raw string (not byte raw) because the JSON contains
        // Japanese characters; `br#"…"#` rejects non-ASCII.
        FIXTURE_JSON.as_bytes()
    }

    const FIXTURE_JSON: &str = r##"{
            "data": {
                "Page": {
                    "media": [
                        {
                            "id": 182205,
                            "idMal": 59970,
                            "title": {
                                "romaji": "Tensei Shitara Slime Datta Ken 4th Season",
                                "english": "That Time I Got Reincarnated as a Slime Season 4",
                                "native": "転生したらスライムだった件 第4期",
                                "userPreferred": "Tensei Shitara Slime Datta Ken 4th Season"
                            },
                            "coverImage": {
                                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx182205-q.jpg",
                                "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx182205-q.jpg",
                                "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx182205-q.jpg",
                                "color": "#1abbd6"
                            },
                            "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/182205-f.jpg",
                            "status": "RELEASING",
                            "episodes": null,
                            "trending": 273,
                            "averageScore": 80
                        },
                        {
                            "id": 21,
                            "idMal": 21,
                            "title": {
                                "romaji": "ONE PIECE",
                                "english": "ONE PIECE",
                                "native": "ONE PIECE",
                                "userPreferred": "ONE PIECE"
                            },
                            "coverImage": {
                                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx21-E.jpg",
                                "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx21-E.jpg",
                                "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx21-E.jpg",
                                "color": "#e49335"
                            },
                            "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/21-w.jpg",
                            "status": "RELEASING",
                            "episodes": null,
                            "trending": 167,
                            "averageScore": 87
                        },
                        {
                            "id": 999999,
                            "idMal": null,
                            "title": {
                                "romaji": "Hypothetical Show With No MAL Id",
                                "english": null,
                                "native": null,
                                "userPreferred": "Hypothetical Show With No MAL Id"
                            },
                            "coverImage": {
                                "extraLarge": null,
                                "large": null,
                                "medium": null,
                                "color": null
                            },
                            "bannerImage": null,
                            "status": null,
                            "episodes": null,
                            "trending": null,
                            "averageScore": null
                        }
                    ]
                }
            }
        }"##;

    #[test]
    fn parse_trending_yields_expected_count_and_first_entry() {
        let v = parse_trending(fixture_body()).expect("parses");
        assert_eq!(v.len(), 3);
        let first = &v[0];
        assert_eq!(first.id, 182205);
        assert_eq!(first.id_mal, Some(59970));
        assert_eq!(
            first.title.user_preferred.as_deref(),
            Some("Tensei Shitara Slime Datta Ken 4th Season")
        );
        assert_eq!(first.cover_image.color.as_deref(), Some("#1abbd6"));
        assert!(first.banner_image.is_some());
        assert_eq!(first.status.as_deref(), Some("RELEASING"));
        assert_eq!(first.trending, Some(273));
    }

    #[test]
    fn parse_trending_handles_missing_optionals() {
        let v = parse_trending(fixture_body()).expect("parses");
        let third = &v[2];
        assert_eq!(third.id, 999999);
        assert_eq!(third.id_mal, None);
        assert_eq!(third.title.english, None);
        assert_eq!(third.cover_image.extra_large, None);
        assert_eq!(third.banner_image, None);
        assert_eq!(third.status, None);
        assert_eq!(third.episodes, None);
        assert_eq!(third.trending, None);
        assert_eq!(third.average_score, None);
    }

    #[test]
    fn parse_trending_rejects_html_or_garbage() {
        let r = parse_trending(b"<html>not json</html>");
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[tokio::test]
    async fn trending_makes_correct_post_request() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .and(wiremock::matchers::header(
                "content-type",
                "application/json",
            ))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "query": TRENDING_GQL,
                "variables": { "perPage": 5 },
            })))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(fixture_body()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let v = trending(&client, 5, Some(&server.uri())).await.expect("ok");
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].id, 182205);
    }

    #[tokio::test]
    async fn trending_propagates_5xx_as_upstream() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    #[tokio::test]
    async fn trending_surfaces_429_for_rate_limit() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 429 }));
    }
}
