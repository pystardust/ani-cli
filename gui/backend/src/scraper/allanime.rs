//! allanime search GraphQL — see `super` for the architectural rationale.

use serde::Deserialize;
use url::Url;

use crate::error::{AniError, Result};

/// One candidate row from allanime's search response. Mirrors the
/// fields ani-cli pulls in `search_anime` (`_id`, `name`,
/// `availableEpisodes`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Candidate {
    /// allanime's internal show id.
    #[serde(rename = "_id")]
    pub id: String,
    /// Display name (the same one ani-cli would show in fzf).
    pub name: String,
    /// Episode counts per translation type. Sub is what `ani-cli`'s
    /// default mode reads; dub is also exposed for `--dub` plays.
    #[serde(default, rename = "availableEpisodes")]
    pub available_episodes: AvailableEpisodes,
}

/// `availableEpisodes` object from allanime's response. Both fields
/// default to 0 when allanime omits them (rare but possible).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AvailableEpisodes {
    /// Subtitled-episode count.
    #[serde(default)]
    pub sub: u32,
    /// Dubbed-episode count.
    #[serde(default)]
    pub dub: u32,
}

impl AvailableEpisodes {
    /// Episode count to score against Kitsu's `episode_count`. Picks
    /// the dub count when the caller asked for dub playback, else sub.
    #[must_use]
    pub fn for_mode(&self, mode: &str) -> u32 {
        if mode == "dub" {
            self.dub
        } else {
            self.sub
        }
    }
}

/// Pick the 1-based index of the candidate whose episode count for
/// `mode` is closest to `expected`. Returns `None` when the input is
/// empty. Ties resolve in favour of the earliest candidate (allanime's
/// own ranking) so a perfect-match-vs-perfect-match never reorders.
///
/// Returns a 1-based index because ani-cli's `-S` flag is 1-based.
#[must_use]
pub fn pick_by_ep_count(candidates: &[Candidate], expected: u32, mode: &str) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    let mut best_idx = 0usize;
    let mut best_dist = u32::MAX;
    for (i, c) in candidates.iter().enumerate() {
        let got = c.available_episodes.for_mode(mode);
        let dist = got.abs_diff(expected);
        if dist < best_dist {
            best_idx = i;
            best_dist = dist;
        }
    }
    Some(best_idx + 1)
}

const ALLANIME_API: &str = "https://api.allanime.day";
const ALLANIME_REFERER: &str = "https://allmanga.to";

/// Subset of allanime's `show(_id: …)` response — only the title fields
/// our resolver consumes when bridging from a history-recorded
/// allmanga show_id to a Kitsu entry.
///
/// The `name` field can be a stub (e.g. `"1P"` for One Piece, `"Nato:
/// Shippuuden"` for Naruto Shippuuden) — those are the cases where
/// title-text-search through Kitsu returns zero hits and the home
/// page's Continue Watching card falls through to the bare allmanga
/// label. `english_name` / `native_name` / `alt_names` are the
/// recovery surface.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ShowMetadata {
    /// Primary catalogue name. Sometimes a stub.
    #[serde(default)]
    pub name: String,
    /// Localised English title. `null` on shows that don't ship one.
    #[serde(default, rename = "englishName")]
    pub english_name: Option<String>,
    /// Romanised native-language title. `null` on non-Japanese shows.
    #[serde(default, rename = "nativeName")]
    pub native_name: Option<String>,
    /// Alternate titles allmanga keeps for fuzzy search. May be empty
    /// or contain non-Latin scripts; callers filter as needed.
    #[serde(default, rename = "altNames")]
    pub alt_names: Vec<String>,
}

impl ShowMetadata {
    /// Ordered list of search terms to feed to a downstream fuzzy
    /// matcher (Kitsu text search). `english_name` first because Kitsu
    /// indexes by transliterated English titles; `native_name` second
    /// for shows whose English release is the alias; `alt_names` last
    /// as a wide net. `name` is intentionally NOT included — it's the
    /// stub that already failed the original search, so retrying it is
    /// a no-op. Empty/whitespace-only strings are skipped.
    #[must_use]
    pub fn search_terms(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for raw in std::iter::once(self.english_name.as_deref())
            .chain(std::iter::once(self.native_name.as_deref()))
            .chain(self.alt_names.iter().map(|s| Some(s.as_str())))
        {
            if let Some(s) = raw {
                let trimmed = s.trim();
                if !trimmed.is_empty() && !out.iter().any(|prev| prev == trimmed) {
                    out.push(trimmed.to_string());
                }
            }
        }
        out
    }
}

const SHOW_GQL: &str =
    "query Show($showId: String!){ show(_id: $showId){ name englishName nativeName altNames }}";

/// Fetch allanime's per-show metadata (title aliases) for a given
/// `show_id`. Returns the parsed [`ShowMetadata`] on a 2xx response
/// with the expected shape.
///
/// `base_override` mirrors the `search()` parameter — `None` in prod,
/// `Some(uri)` in tests pointing at wiremock.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the JSON body doesn't shape into
///   `{ data: { show: {...} } }`.
pub async fn fetch_show(
    _client: &reqwest::Client,
    _show_id: &str,
    _base_override: Option<&str>,
) -> Result<ShowMetadata> {
    // STUB (red commit). Real impl lands in the green commit; tests
    // in this module assert the contract.
    Ok(ShowMetadata::default())
}

/// Replace ASCII space with `+` to match ani-cli's `search_anime`
/// pre-processing (line ~178: `printf '%s' "$1" | sed 's| |+|g'`).
/// Allanime treats `+` as a literal character in the search query,
/// so a clean-spaces query and a plus-joined query return *different*
/// hit lists. Both layers must agree byte-for-byte or our index pick
/// won't line up with what ani-cli sees — Stone Ocean Part 2
/// reproduces this when our scraper saw 11 hits and ani-cli saw 2.
///
/// No further URL-encoding is applied; ani-cli doesn't either, and
/// allanime's GraphQL accepts the field as JSON-stringified text.
#[must_use]
pub fn encode_query_for_allanime(s: &str) -> String {
    s.replace(' ', "+")
}
const SEARCH_GQL: &str = "query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name availableEpisodes __typename } }}";

/// Hit allanime's GraphQL `shows.search` endpoint with the same
/// payload ani-cli would send and return the candidate list. `mode`
/// is `"sub"` or `"dub"`; passed through as the `translationType`
/// variable.
///
/// `base_override` lets tests redirect the call at a wiremock server.
/// In prod, callers pass `None`.
///
/// # Errors
/// - [`AniError::Network`] for connection failures
/// - [`AniError::Upstream`] for non-2xx responses
/// - [`AniError::ParseFailed`] when the JSON body doesn't shape into
///   the expected `Candidate` list
pub async fn search(
    client: &reqwest::Client,
    query: &str,
    mode: &str,
    base_override: Option<&str>,
) -> Result<Vec<Candidate>> {
    let base = base_override.unwrap_or(ALLANIME_API);
    let url = format!("{base}/api");
    let _ = Url::parse(&url).map_err(|_| AniError::ParseFailed {
        detail: format!("allanime search url: {url}"),
    })?;

    // Body shape mirrors ani-cli's `search_anime` POST byte-for-byte —
    // including the space→`+` substitution. See encode_query_for_allanime
    // for why; without it our hit list disagrees with ani-cli's and our
    // index pick lands on a candidate ani-cli's `-S N` can't reach.
    let encoded_query = encode_query_for_allanime(query);
    let body = serde_json::json!({
        "variables": {
            "search": {
                "allowAdult": false,
                "allowUnknown": false,
                "query": encoded_query,
            },
            "limit": 40,
            "page": 1,
            "translationType": mode,
            "countryOrigin": "ALL",
        },
        "query": SEARCH_GQL,
    });

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("referer", ALLANIME_REFERER)
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

    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        shows: Shows,
    }
    #[derive(Deserialize)]
    struct Shows {
        edges: Vec<Candidate>,
    }
    let parsed: Wrap = resp.json().await.map_err(|e| AniError::ParseFailed {
        detail: format!("allanime search response: {e}"),
    })?;
    Ok(parsed.data.shows.edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, name: &str, sub: u32) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            available_episodes: AvailableEpisodes { sub, dub: 0 },
        }
    }

    #[test]
    fn pick_by_ep_count_returns_none_for_empty_input() {
        assert_eq!(pick_by_ep_count(&[], 500, "sub"), None);
    }

    #[test]
    fn pick_by_ep_count_chooses_closest_to_expected() {
        // The Naruto: Shippuden repro. allanime's ranking puts the
        // side story first; we prefer the show whose ep_count is
        // closest to Kitsu's 500.
        let cands = vec![
            cand(
                "side-story",
                "Naruto: Shippuuden: Shippuu! Konoha Gakuen Den",
                1,
            ),
            cand("main", "Naruto: Shippuuden", 500),
            cand("ova", "Naruto OVAs", 12),
        ];
        assert_eq!(pick_by_ep_count(&cands, 500, "sub"), Some(2));
    }

    #[test]
    fn pick_by_ep_count_returns_one_when_only_one_candidate() {
        let cands = vec![cand("only", "Some Show", 12)];
        assert_eq!(pick_by_ep_count(&cands, 500, "sub"), Some(1));
    }

    #[test]
    fn pick_by_ep_count_breaks_ties_in_allanime_order() {
        // Two candidates equidistant from expected — the earlier one
        // wins to preserve allanime's own relevance ranking when the
        // ep_count signal is ambiguous.
        let cands = vec![cand("a", "A", 100), cand("b", "B", 100)];
        assert_eq!(pick_by_ep_count(&cands, 100, "sub"), Some(1));
    }

    #[test]
    fn pick_by_ep_count_uses_dub_when_mode_is_dub() {
        let cands = vec![
            Candidate {
                id: "a".into(),
                name: "A".into(),
                available_episodes: AvailableEpisodes { sub: 500, dub: 1 },
            },
            Candidate {
                id: "b".into(),
                name: "B".into(),
                available_episodes: AvailableEpisodes { sub: 1, dub: 500 },
            },
        ];
        // Looking for 500 dub-eps: B wins (dub=500), even though A
        // would win for sub.
        assert_eq!(pick_by_ep_count(&cands, 500, "dub"), Some(2));
    }

    #[tokio::test]
    async fn search_parses_allanime_graphql_response() {
        // Body shape from a real allanime response. Wiremock returns
        // it; the parser pulls out the edges array verbatim.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "shows": {
                    "edges": [
                        {
                            "_id": "abc",
                            "name": "Naruto: Shippuuden",
                            "availableEpisodes": {"sub": 500, "dub": 209, "raw": 0},
                            "__typename": "Show"
                        },
                        {
                            "_id": "side",
                            "name": "Naruto: Shippuuden: Konoha",
                            "availableEpisodes": {"sub": 1, "dub": 0, "raw": 0},
                            "__typename": "Show"
                        }
                    ]
                }
            }
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let cands = search(&client, "Naruto: Shippuuden", "sub", Some(&server.uri()))
            .await
            .expect("search ok");
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].id, "abc");
        assert_eq!(cands[0].available_episodes.sub, 500);
        assert_eq!(cands[1].available_episodes.sub, 1);
    }

    #[test]
    fn encode_query_for_allanime_replaces_spaces_with_plus() {
        // Drift-critical: ani-cli does `printf '%s' "$1" | sed 's| |+|g'`
        // before posting the GraphQL query (line ~178). Allanime treats
        // `+` as a literal character, so a clean-spaces query and a
        // plus-joined query return *different* hit lists. When our
        // scraper search disagrees with ani-cli's own search, our
        // pick_by_ep_count picks an index that ani-cli's `-S N` can't
        // reach (Stone Ocean Part 2 reproduces this — we saw 11 hits
        // and picked 3, ani-cli saw 2 hits and exited because index 3
        // is out of range).
        assert_eq!(
            encode_query_for_allanime("JoJo no Kimyou na Bouken: Stone Ocean Part 2"),
            "JoJo+no+Kimyou+na+Bouken:+Stone+Ocean+Part+2"
        );
        assert_eq!(encode_query_for_allanime(""), "");
        assert_eq!(encode_query_for_allanime("nospace"), "nospace");
        // Multiple consecutive spaces collapse one-to-one (mirrors
        // ani-cli's sed behaviour, which doesn't squeeze).
        assert_eq!(encode_query_for_allanime("a  b"), "a++b");
    }

    #[tokio::test]
    async fn search_returns_upstream_error_on_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = search(&client, "x", "sub", Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    // — fetch_show: bridge from cryptic allmanga `name` (e.g. "1P" for
    //   One Piece) to richer englishName/altNames the resolver feeds
    //   to Kitsu's text search.

    #[tokio::test]
    async fn fetch_show_parses_name_english_native_and_alt_names() {
        // Real shape lifted from allanime's response for One Piece
        // (show_id ReooPAxPMsHM4KPMY). `name` is the stub the CLI
        // writes to ani-hsts; the rest are recovery surfaces.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "show": {
                    "name": "1P",
                    "englishName": "One Piece",
                    "nativeName": "ONE PIECE",
                    "altNames": ["One Piece", "海贼王", "ワンピース"]
                }
            }
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let show = fetch_show(&client, "ReooPAxPMsHM4KPMY", Some(&server.uri()))
            .await
            .expect("fetch ok");
        assert_eq!(show.name, "1P");
        assert_eq!(show.english_name.as_deref(), Some("One Piece"));
        assert_eq!(show.native_name.as_deref(), Some("ONE PIECE"));
        assert_eq!(
            show.alt_names,
            vec![
                "One Piece".to_string(),
                "海贼王".to_string(),
                "ワンピース".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn fetch_show_returns_upstream_error_on_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = fetch_show(&client, "x", Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    #[tokio::test]
    async fn fetch_show_handles_null_show_as_empty_metadata() {
        // Allanime returns `data.show: null` for unknown ids. Treat as
        // empty (no aliases to enrich) instead of erroring out — the
        // caller will skip the enrichment and fall through.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({ "data": { "show": null } });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let show = fetch_show(&client, "missing", Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(show.name, "");
        assert_eq!(show.english_name, None);
        assert!(show.alt_names.is_empty());
    }

    #[test]
    fn search_terms_walks_english_then_native_then_alt_names() {
        let show = ShowMetadata {
            name: "1P".into(),
            english_name: Some("One Piece".into()),
            native_name: Some("ONE PIECE".into()),
            alt_names: vec!["One Piece".into(), "海贼王".into()],
        };
        // english_name first, native_name second, then alt_names —
        // dedupe so the duplicate "One Piece" doesn't appear twice.
        // `name` is excluded (it already failed the original search).
        assert_eq!(
            show.search_terms(),
            vec![
                "One Piece".to_string(),
                "ONE PIECE".to_string(),
                "海贼王".to_string()
            ]
        );
    }

    #[test]
    fn search_terms_skips_empty_and_whitespace_strings() {
        let show = ShowMetadata {
            name: "stub".into(),
            english_name: Some("".into()),
            native_name: Some("   ".into()),
            alt_names: vec!["".into(), "Real Title".into()],
        };
        assert_eq!(show.search_terms(), vec!["Real Title".to_string()]);
    }
}
