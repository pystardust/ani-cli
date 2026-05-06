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

    // Body shape mirrors ani-cli's `search_anime` POST byte-for-byte.
    let body = serde_json::json!({
        "variables": {
            "search": {
                "allowAdult": false,
                "allowUnknown": false,
                "query": query,
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
}
