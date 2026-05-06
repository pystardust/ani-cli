//! Kitsu JSON:API client and parser.
//!
//! Two layers: pure functions [`parse_search_response`] / [`parse_anime_response`]
//! that decode bytes into [`KitsuAnimeRef`] (fixture-driven unit tests), and an
//! async [`KitsuClient`] that wraps the shared `reqwest::Client` (wiremock
//! integration tests).
//!
//! Kitsu returns:
//! - `posterImage` — 5:7 portrait, keys `tiny / small / medium / large / original`
//! - `coverImage`  — 21:5 banner, keys `tiny / small / large / original` (no `medium`)
//! - both can be `null` on the wire; ~50% of currently-airing top results have
//!   `coverImage: null` per ad-hoc inspection. UI handles fallback.
//!
//! `averageRating` arrives as a string (e.g. `"83.98"`) and is parsed to
//! `f32` here so callers can compute / compare without re-parsing.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

/// Kitsu API base URL. Override in tests via [`KitsuClient::with_base`].
pub const KITSU_BASE: &str = "https://kitsu.io/api/edge";

/// Sparse fieldset we ask Kitsu to return. Listed verbatim so the fixture
/// HTTP requests in tests can match the same string.
pub const ANIME_FIELDS: &str = "canonicalTitle,titles,slug,synopsis,startDate,endDate,episodeCount,averageRating,subtype,status,posterImage,coverImage,ageRating,popularityRank";

/// Sparse fieldset for the episode resource. Same convention as
/// [`ANIME_FIELDS`] — kept verbatim so wiremock can match exactly.
pub const EPISODE_FIELDS: &str =
    "canonicalTitle,seasonNumber,number,relativeNumber,length,synopsis,airdate,thumbnail";

/// Public, framework-free Kitsu anime view. Mirrors the attributes our UI
/// consumes — search hits and detail responses share this shape because
/// our `fields[anime]` request asks for the same set in both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitsuAnimeRef {
    /// Stringified Kitsu anime id (e.g. `"12"` for One Piece).
    pub id: String,
    /// Title Kitsu considers canonical (often the romanized Japanese form).
    pub canonical_title: String,
    /// URL slug Kitsu uses on its public site (`kitsu.io/anime/<slug>`).
    pub slug: Option<String>,
    /// Long-form synopsis. Often several paragraphs.
    pub synopsis: Option<String>,
    /// Start of broadcast as `YYYY-MM-DD` (Kitsu's wire format).
    pub start_date: Option<String>,
    /// End of broadcast as `YYYY-MM-DD`. Null while currently airing.
    pub end_date: Option<String>,
    /// Total episode count when known. Null for ongoing series.
    pub episode_count: Option<u32>,
    /// Aggregate rating on Kitsu's 0–100 scale (Kitsu serializes as
    /// string; we parse to `f32`). Null when too few ratings exist.
    pub average_rating: Option<f32>,
    /// `TV`, `movie`, `OVA`, `special`, etc.
    pub subtype: Option<String>,
    /// `current`, `finished`, `tba`, `unreleased`, etc.
    pub status: Option<String>,
    /// Content rating (`G`, `PG`, `R`, etc.) when assigned.
    pub age_rating: Option<String>,
    /// Kitsu's popularity rank (1 = most popular).
    pub popularity_rank: Option<u32>,
    /// Portrait poster URLs (5:7). Always present in our experience.
    pub poster_image: Option<KitsuPosterImage>,
    /// Banner cover URLs (21:5). Often null — UI must fall back.
    pub cover_image: Option<KitsuCoverImage>,
}

/// 5:7 portrait poster URLs at the Kitsu-rendered sizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuPosterImage {
    /// 110×156 thumbnail.
    pub tiny: Option<String>,
    /// 284×402 small.
    pub small: Option<String>,
    /// 390×554 medium.
    pub medium: Option<String>,
    /// 550×780 large — what most card layouts use.
    pub large: Option<String>,
    /// Source-resolution upload, no resampling.
    pub original: Option<String>,
}

/// 21:5 banner cover URLs at the Kitsu-rendered sizes. Note the absence
/// of `medium` — Kitsu doesn't expose that variant for covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuCoverImage {
    /// 1840×440 tiny.
    pub tiny: Option<String>,
    /// 2208×528 small.
    pub small: Option<String>,
    /// 3360×800 large — what hero banners use.
    pub large: Option<String>,
    /// Source-resolution upload, no resampling.
    pub original: Option<String>,
}

/// One episode in a Kitsu anime's episode list. Kitsu only renders the
/// `original` size for thumbnails — no tiny/small variants like for the
/// poster + cover. The frontend can downscale via the image-cache layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuEpisode {
    /// Stringified Kitsu episode id (e.g. `"103482"` for One Piece ep 1).
    pub id: String,
    /// Title Kitsu considers canonical for this episode.
    pub canonical_title: Option<String>,
    /// Season this episode belongs to. `1` for shows that don't break
    /// into multiple seasons in Kitsu's data model.
    pub season_number: Option<u32>,
    /// Overall episode number across the show (`1`-based).
    pub number: Option<u32>,
    /// Episode number within the season (`1`-based).
    pub relative_number: Option<u32>,
    /// Length in minutes. Null for unaired or unknown.
    pub length: Option<u32>,
    /// Long-form description of the episode. Spoiler-heavy — UIs may
    /// want to gate behind a "show synopsis" toggle.
    pub synopsis: Option<String>,
    /// Airdate as `YYYY-MM-DD`.
    pub airdate: Option<String>,
    /// Thumbnail still — only `original` is exposed by Kitsu.
    pub thumbnail: Option<KitsuEpisodeThumbnail>,
}

/// Single-size thumbnail Kitsu exposes for episodes. Unlike posters
/// + covers, no tiny/small/large variants — just the original upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuEpisodeThumbnail {
    /// Source-resolution upload (Kitsu doesn't resample).
    pub original: Option<String>,
}

// --- Wire types (private to this module) ---------------------------------

#[derive(Deserialize)]
struct ApiList<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct ApiSingle<T> {
    data: T,
}

#[derive(Deserialize)]
struct AnimeResource {
    id: String,
    attributes: AnimeAttributes,
}

#[derive(Deserialize)]
struct EpisodeResource {
    id: String,
    attributes: EpisodeAttributes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeAttributes {
    canonical_title: Option<String>,
    season_number: Option<u32>,
    number: Option<u32>,
    relative_number: Option<u32>,
    length: Option<u32>,
    synopsis: Option<String>,
    airdate: Option<String>,
    thumbnail: Option<KitsuEpisodeThumbnail>,
}

fn into_episode(r: EpisodeResource) -> KitsuEpisode {
    KitsuEpisode {
        id: r.id,
        canonical_title: r.attributes.canonical_title,
        season_number: r.attributes.season_number,
        number: r.attributes.number,
        relative_number: r.attributes.relative_number,
        length: r.attributes.length,
        synopsis: r.attributes.synopsis,
        airdate: r.attributes.airdate,
        thumbnail: r.attributes.thumbnail,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimeAttributes {
    canonical_title: Option<String>,
    slug: Option<String>,
    synopsis: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    episode_count: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_optional_f32_string")]
    average_rating: Option<f32>,
    subtype: Option<String>,
    status: Option<String>,
    age_rating: Option<String>,
    popularity_rank: Option<u32>,
    poster_image: Option<KitsuPosterImage>,
    cover_image: Option<KitsuCoverImage>,
}

fn deserialize_optional_f32_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s.parse::<f32>().map(Some).map_err(serde::de::Error::custom),
    }
}

fn into_ref(r: AnimeResource) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: r.id,
        canonical_title: r.attributes.canonical_title.unwrap_or_default(),
        slug: r.attributes.slug,
        synopsis: r.attributes.synopsis,
        start_date: r.attributes.start_date,
        end_date: r.attributes.end_date,
        episode_count: r.attributes.episode_count,
        average_rating: r.attributes.average_rating,
        subtype: r.attributes.subtype,
        status: r.attributes.status,
        age_rating: r.attributes.age_rating,
        popularity_rank: r.attributes.popularity_rank,
        poster_image: r.attributes.poster_image,
        cover_image: r.attributes.cover_image,
    }
}

// --- Pure parsers --------------------------------------------------------

/// Parse `{ "data": [...] }` into a list of refs. Used for search.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the body isn't valid JSON:API for
/// an anime collection.
pub fn parse_search_response(body: &[u8]) -> Result<Vec<KitsuAnimeRef>> {
    let parsed: ApiList<AnimeResource> =
        serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
            detail: format!("kitsu search parse: {e}"),
        })?;
    Ok(parsed.data.into_iter().map(into_ref).collect())
}

/// Parse `{ "data": {...} }` into a single ref. Used for `/anime/:id`.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the body isn't valid JSON:API for
/// a single anime resource.
pub fn parse_anime_response(body: &[u8]) -> Result<KitsuAnimeRef> {
    let parsed: ApiSingle<AnimeResource> =
        serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
            detail: format!("kitsu detail parse: {e}"),
        })?;
    Ok(into_ref(parsed.data))
}

/// Parse `{ "data": [...] }` into a list of episodes. Used for
/// `/anime/:id/episodes`.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't valid JSON:API
/// for an episode collection.
pub fn parse_episodes_response(body: &[u8]) -> Result<Vec<KitsuEpisode>> {
    let parsed: ApiList<EpisodeResource> =
        serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
            detail: format!("kitsu episodes parse: {e}"),
        })?;
    Ok(parsed.data.into_iter().map(into_episode).collect())
}

// --- Async client --------------------------------------------------------

/// Async wrapper around `reqwest::Client` that handles Kitsu's URL shape.
#[derive(Debug, Clone)]
pub struct KitsuClient {
    http: reqwest::Client,
    base: String,
}

impl KitsuClient {
    /// Build a client pointing at the live Kitsu API.
    #[must_use]
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_base(http, KITSU_BASE)
    }

    /// Build a client pointing at an arbitrary base URL (e.g. wiremock).
    pub fn with_base(http: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }

    /// Search for anime by free-text. `limit` caps `page[limit]`.
    ///
    /// # Errors
    /// - [`AniError::Upstream`] on non-2xx HTTP response.
    /// - [`AniError::Network`] on transport failure.
    /// - [`AniError::ParseFailed`] on malformed JSON:API.
    pub async fn search(&self, query: &str, limit: u8) -> Result<Vec<KitsuAnimeRef>> {
        let resp = self
            .http
            .get(format!("{}/anime", self.base))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[
                ("filter[text]", query.to_string()),
                ("page[limit]", limit.to_string()),
                ("fields[anime]", ANIME_FIELDS.to_string()),
            ])
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_search_response(&body)
    }

    /// Currently-airing anime sorted by user count descending — a usable
    /// proxy for "trending" until the AniList client lands. `limit` caps
    /// `page[limit]`.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn currently_airing_by_user_count(&self, limit: u8) -> Result<Vec<KitsuAnimeRef>> {
        self.list(&[
            ("filter[status]", "current".to_string()),
            ("sort", "-userCount".to_string()),
            ("page[limit]", limit.to_string()),
            ("fields[anime]", ANIME_FIELDS.to_string()),
        ])
        .await
    }

    /// Top-rated anime above the noise floor (averageRating ≥ 70/100).
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn top_rated(&self, limit: u8) -> Result<Vec<KitsuAnimeRef>> {
        self.list(&[
            ("filter[averageRating]", "70..".to_string()),
            ("sort", "-averageRating".to_string()),
            ("page[limit]", limit.to_string()),
            ("fields[anime]", ANIME_FIELDS.to_string()),
        ])
        .await
    }

    async fn list(&self, params: &[(&str, String)]) -> Result<Vec<KitsuAnimeRef>> {
        let resp = self
            .http
            .get(format!("{}/anime", self.base))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(params)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_search_response(&body)
    }

    /// Fetch a page of episodes for an anime, sorted by absolute number
    /// ascending. `page` is 1-based; the Kitsu offset is computed as
    /// `(page - 1) * limit`.
    ///
    /// # Errors
    /// Same as [`Self::search`] / [`Self::anime_detail`].
    pub async fn episodes(
        &self,
        anime_id: &str,
        page: u32,
        limit: u8,
    ) -> Result<Vec<KitsuEpisode>> {
        let offset = page.saturating_sub(1).saturating_mul(u32::from(limit));
        let resp = self
            .http
            .get(format!("{}/anime/{}/episodes", self.base, anime_id))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[
                ("page[limit]", limit.to_string()),
                ("page[offset]", offset.to_string()),
                ("fields[episodes]", EPISODE_FIELDS.to_string()),
                ("sort", "number".to_string()),
            ])
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_episodes_response(&body)
    }

    /// Fetch a single anime by Kitsu id.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn anime_detail(&self, id: &str) -> Result<KitsuAnimeRef> {
        let resp = self
            .http
            .get(format!("{}/anime/{}", self.base, id))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[("fields[anime]", ANIME_FIELDS.to_string())])
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_anime_response(&body)
    }

    /// Look up an anime by its slug — Kitsu's URL-stable identifier.
    /// Used as a fallback when the text search doesn't include a known
    /// sequel in its results (Kitsu's `filter[text]` ranks the most-
    /// popular sibling above all alternates and sometimes drops them
    /// from the page entirely; see Stone Ocean Part 2).
    ///
    /// Returns `Ok(None)` when no entry matches the slug; `Err` only
    /// for upstream / network / parse failures.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn anime_by_slug(&self, slug: &str) -> Result<Option<KitsuAnimeRef>> {
        let resp = self
            .http
            .get(format!("{}/anime", self.base))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[
                ("filter[slug]", slug.to_string()),
                ("page[limit]", "1".to_string()),
                ("fields[anime]", ANIME_FIELDS.to_string()),
            ])
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        let hits = parse_search_response(&body)?;
        Ok(hits.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/search_one_piece.json");
    const DETAIL_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/anime_one_piece_detail.json");
    const NULL_COVER_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/anime_null_cover_detail.json");
    const EPISODES_FIXTURE: &[u8] =
        include_bytes!("../../../../tests/fixtures/kitsu/episodes_one_piece.json");

    #[test]
    fn parse_search_returns_all_hits_with_canonical_titles() {
        let hits = parse_search_response(SEARCH_FIXTURE).expect("parses");
        assert_eq!(hits.len(), 5, "fixture has 5 search results");
        // First result is anime/12 = One Piece (canonical).
        let first = &hits[0];
        assert_eq!(first.id, "12");
        assert_eq!(first.canonical_title, "One Piece");
        assert_eq!(first.subtype.as_deref(), Some("TV"));
        assert_eq!(first.status.as_deref(), Some("current"));
    }

    #[test]
    fn parse_search_decodes_average_rating_string_into_f32() {
        let hits = parse_search_response(SEARCH_FIXTURE).expect("parses");
        let one_piece = &hits[0];
        // "83.98" on the wire → f32 in our type.
        let r = one_piece.average_rating.expect("rating present");
        assert!((r - 83.98).abs() < 0.01, "got {r}");
    }

    #[test]
    fn parse_search_handles_null_cover_image() {
        let hits = parse_search_response(SEARCH_FIXTURE).expect("parses");
        // anime/13623 in this fixture has coverImage: null.
        let null_cover = hits
            .iter()
            .find(|h| h.id == "13623")
            .expect("13623 is in fixture");
        assert!(
            null_cover.cover_image.is_none(),
            "null cover deserializes to None, got {:?}",
            null_cover.cover_image
        );
        // Poster image is still present.
        assert!(
            null_cover.poster_image.is_some(),
            "poster present even when cover is null"
        );
    }

    #[test]
    fn parse_search_extracts_poster_and_cover_urls_when_present() {
        let hits = parse_search_response(SEARCH_FIXTURE).expect("parses");
        let one_piece = &hits[0];
        let poster = one_piece.poster_image.as_ref().expect("poster present");
        let cover = one_piece.cover_image.as_ref().expect("cover present");
        assert!(poster
            .large
            .as_deref()
            .unwrap_or("")
            .starts_with("https://media.kitsu.app/"));
        assert!(cover
            .large
            .as_deref()
            .unwrap_or("")
            .starts_with("https://media.kitsu.app/"));
        // Cover has no `medium` key — KitsuCoverImage has no medium field,
        // so this is enforced at the type level. Sanity:
        let json = serde_json::to_value(cover).unwrap();
        assert!(json.get("medium").is_none());
    }

    #[test]
    fn parse_anime_detail_decodes_single_resource() {
        let detail = parse_anime_response(DETAIL_FIXTURE).expect("parses");
        assert_eq!(detail.id, "12");
        assert_eq!(detail.canonical_title, "One Piece");
        assert!(
            detail.synopsis.as_deref().unwrap_or("").len() > 100,
            "synopsis is real text"
        );
        assert_eq!(detail.start_date.as_deref(), Some("1999-10-20"));
    }

    #[test]
    fn parse_anime_detail_with_null_cover_returns_none_for_cover() {
        let detail = parse_anime_response(NULL_COVER_FIXTURE).expect("parses");
        assert!(detail.cover_image.is_none(), "null cover → None");
        assert!(detail.poster_image.is_some(), "poster still present");
    }

    #[test]
    fn parse_search_rejects_non_jsonapi_body() {
        let r = parse_search_response(b"<html>not json</html>");
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[test]
    fn parse_search_rejects_data_object_when_expecting_array() {
        let body = br#"{"data":{"id":"12","attributes":{}}}"#;
        let r = parse_search_response(body);
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[test]
    fn parse_anime_detail_rejects_data_array_when_expecting_object() {
        let body = br#"{"data":[]}"#;
        let r = parse_anime_response(body);
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[test]
    fn parse_episodes_returns_all_with_canonical_titles() {
        let eps = parse_episodes_response(EPISODES_FIXTURE).expect("parses");
        assert_eq!(eps.len(), 12, "fixture has 12 episodes");
        let first = &eps[0];
        assert_eq!(first.id, "103482");
        assert_eq!(first.number, Some(1));
        assert_eq!(first.season_number, Some(1));
        assert_eq!(first.length, Some(24));
        assert_eq!(first.airdate.as_deref(), Some("1999-10-20"));
        assert!(first
            .canonical_title
            .as_deref()
            .unwrap_or("")
            .contains("King of the Pirates"));
    }

    #[test]
    fn parse_episodes_extracts_thumbnail_original_when_present() {
        let eps = parse_episodes_response(EPISODES_FIXTURE).expect("parses");
        let first = &eps[0];
        let thumb = first.thumbnail.as_ref().expect("thumbnail present");
        let url = thumb.original.as_deref().unwrap_or_default();
        assert!(
            url.starts_with("https://media.kitsu.app/"),
            "thumbnail.original is a Kitsu CDN URL: {url}"
        );
    }

    #[test]
    fn parse_episodes_handles_null_thumbnail_gracefully() {
        let body = br#"{"data":[{"id":"1","type":"episodes","attributes":{"canonicalTitle":"x","number":1,"thumbnail":null}}]}"#;
        let eps = parse_episodes_response(body).expect("parses");
        assert_eq!(eps.len(), 1);
        assert!(eps[0].thumbnail.is_none());
    }

    #[test]
    fn parse_episodes_rejects_non_jsonapi_body() {
        let r = parse_episodes_response(b"<html>not json</html>");
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[test]
    fn deserialize_optional_f32_string_handles_null_empty_and_invalid() {
        // Helper for testing the custom deserializer in isolation via a wrapper struct.
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "deserialize_optional_f32_string")]
            v: Option<f32>,
        }
        let none: W = serde_json::from_str(r#"{"v":null}"#).unwrap();
        assert_eq!(none.v, None);
        let empty: W = serde_json::from_str(r#"{"v":""}"#).unwrap();
        assert_eq!(empty.v, None);
        let some: W = serde_json::from_str(r#"{"v":"7.5"}"#).unwrap();
        assert!((some.v.unwrap() - 7.5).abs() < 1e-6);
        let bad: std::result::Result<W, _> = serde_json::from_str(r#"{"v":"not-a-number"}"#);
        assert!(bad.is_err());
    }
}
