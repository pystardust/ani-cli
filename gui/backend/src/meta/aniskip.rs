//! aniskip.com client — opening / ending / recap timestamp lookup.
//!
//! Crowdsourced skip times for OP, ED, and "mixed" segments.
//! Used by the embedded player to render Skip OP / Skip Outro
//! buttons (and, when the user opts in via settings, to auto-skip).
//!
//! Endpoint: `GET /v2/skip-times/{mal_id}/{episode}?types[]=op&types[]=ed&episodeLength=N`
//!
//! Response envelope:
//! ```text
//! { found, results: [{ interval: {startTime, endTime}, skipType, skipId, episodeLength }, ...],
//!   message, statusCode }
//! ```
//!
//! 404 from aniskip means "no skip times catalogued" — not an
//! error; we return `Ok(empty Vec)` so the player just doesn't
//! render the skip button.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

const ANISKIP_API: &str = "https://api.aniskip.com";

/// One skip interval — OP, ED, recap, or "mixed-op" / "mixed-ed".
/// `start_time` / `end_time` are seconds (floats; aniskip stores
/// sub-second precision because the timestamps come from frame-
/// accurate user submissions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkipInterval {
    /// Aniskip skip type — `"op"`, `"ed"`, `"mixed-op"`,
    /// `"mixed-ed"`, `"recap"`. Display layer maps to user copy.
    pub skip_type: String,
    pub start_time: f32,
    pub end_time: f32,
}

/// GET aniskip's skip times for `(mal_id, episode)`. Episode
/// length feeds aniskip's per-episode disambiguation (some shows
/// have alternate runtimes for movie cuts vs. TV broadcast).
///
/// Returns an empty Vec when aniskip has no skip times for the
/// requested episode (HTTP 404 or `found: false`); transport /
/// parse failures propagate.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP that isn't 404.
/// - [`AniError::ParseFailed`] on a malformed response shape.
pub async fn fetch_skip_times(
    client: &reqwest::Client,
    mal_id: u32,
    episode: &str,
    episode_length: f32,
    base_override: Option<&str>,
) -> Result<Vec<SkipInterval>> {
    let base = base_override.unwrap_or(ANISKIP_API);
    let url = format!("{base}/v2/skip-times/{mal_id}/{episode}");
    let resp = client
        .get(&url)
        .header("accept", "application/json")
        .query(&[
            ("types[]", "op".to_string()),
            ("types[]", "ed".to_string()),
            ("episodeLength", format!("{episode_length:.0}")),
        ])
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    // 404 = "no skip times catalogued" — aniskip's documented
    // semantic. The body is the same `{found: false, results: []}`
    // envelope so we just feed it through the parser; the empty
    // Vec is the correct frontend signal.
    if status == reqwest::StatusCode::NOT_FOUND {
        let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
        return parse_skip_times(&bytes);
    }
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    parse_skip_times(&bytes)
}

/// Parse the aniskip v2 response body. Tolerant of `found: false`
/// (returns empty Vec) and ignores unknown skip types so the
/// frontend doesn't have to enumerate them.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ found, results: [...] }` envelope.
pub fn parse_skip_times(body: &[u8]) -> Result<Vec<SkipInterval>> {
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        results: Vec<Result_>,
    }
    #[derive(Deserialize)]
    struct Result_ {
        interval: Interval,
        #[serde(rename = "skipType")]
        skip_type: String,
    }
    #[derive(Deserialize)]
    struct Interval {
        #[serde(rename = "startTime")]
        start_time: f32,
        #[serde(rename = "endTime")]
        end_time: f32,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("aniskip response: {e}"),
    })?;
    Ok(parsed
        .results
        .into_iter()
        .map(|r| SkipInterval {
            skip_type: r.skip_type,
            start_time: r.interval.start_time,
            end_time: r.interval.end_time,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real shape lifted from a live probe of /v2/skip-times/21/100.
    /// Two intervals: an OP at start, an ED near the end.
    const HIT_FIXTURE: &str = r##"{
        "found": true,
        "results": [
            {
                "interval": { "startTime": 0.957, "endTime": 90.957 },
                "skipType": "op",
                "skipId": "ecd5c0ff-1630-444a-a5aa-e8fb914d7f27",
                "episodeLength": 1440.167
            },
            {
                "interval": { "startTime": 1325, "endTime": 1440 },
                "skipType": "ed",
                "skipId": "54c04625-e13b-4dab-ae44-2732aa01211d",
                "episodeLength": 1440.167
            }
        ],
        "message": "Successfully found skip times",
        "statusCode": 200
    }"##;

    /// `found: false` envelope — aniskip's "no data" reply.
    const EMPTY_FIXTURE: &str = r##"{
        "found": false,
        "results": [],
        "message": "No skip times found",
        "statusCode": 404
    }"##;

    #[test]
    fn parse_returns_op_and_ed_intervals_in_order() {
        let v = parse_skip_times(HIT_FIXTURE.as_bytes()).expect("parses");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].skip_type, "op");
        assert!((v[0].start_time - 0.957).abs() < 0.01);
        assert!((v[0].end_time - 90.957).abs() < 0.01);
        assert_eq!(v[1].skip_type, "ed");
        assert_eq!(v[1].start_time, 1325.0);
        assert_eq!(v[1].end_time, 1440.0);
    }

    #[test]
    fn parse_handles_found_false_as_empty() {
        let v = parse_skip_times(EMPTY_FIXTURE.as_bytes()).expect("parses");
        assert!(v.is_empty());
    }

    #[test]
    fn parse_rejects_garbage() {
        let r = parse_skip_times(b"<html>nope</html>");
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[tokio::test]
    async fn fetch_hits_the_right_path_and_query() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v2/skip-times/21/100"))
            .and(wiremock::matchers::query_param("types[]", "op"))
            .and(wiremock::matchers::query_param("episodeLength", "1440"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(HIT_FIXTURE))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let v = fetch_skip_times(&client, 21, "100", 1440.0, Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(v.len(), 2);
    }

    #[tokio::test]
    async fn fetch_treats_404_as_empty_not_error() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(404).set_body_string(EMPTY_FIXTURE))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let v = fetch_skip_times(&client, 99999, "1", 1440.0, Some(&server.uri()))
            .await
            .expect("ok despite 404");
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn fetch_propagates_5xx_as_upstream() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = fetch_skip_times(&client, 21, "1", 1440.0, Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }
}
