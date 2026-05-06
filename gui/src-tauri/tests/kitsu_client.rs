//! Integration tests for the async Kitsu client.
//!
//! Locks two contracts:
//! 1. The HTTP request shape (path, query string, Accept header) matches what
//!    real Kitsu expects — wiremock asserts the request matches `/anime` with
//!    `filter[text]=`, `page[limit]=`, `fields[anime]=` query parameters.
//! 2. End-to-end decoding: the bytes wiremock returns (real Kitsu fixture
//!    JSON) round-trip into `KitsuAnimeRef` instances.

use ani_gui::meta::kitsu::{KitsuClient, ANIME_FIELDS};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SEARCH_FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/search_one_piece.json");
const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

#[tokio::test]
async fn search_sends_jsonapi_accept_and_filter_query() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(header("accept", "application/vnd.api+json"))
        .and(query_param("filter[text]", "one piece"))
        .and(query_param("page[limit]", "5"))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(SEARCH_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let hits = client.search("one piece", 5).await.expect("ok");

    assert_eq!(hits.len(), 5);
    assert_eq!(hits[0].id, "12");
    assert_eq!(hits[0].canonical_title, "One Piece");
}

#[tokio::test]
async fn anime_detail_hits_path_with_id_and_decodes_body() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .and(header("accept", "application/vnd.api+json"))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let detail = client.anime_detail("12").await.expect("ok");

    assert_eq!(detail.id, "12");
    assert_eq!(detail.canonical_title, "One Piece");
    assert_eq!(detail.start_date.as_deref(), Some("1999-10-20"));
}

#[tokio::test]
async fn search_propagates_upstream_status_on_5xx() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let r = client.search("anything", 5).await;
    match r {
        Err(ani_gui::error::AniError::Upstream { status }) => assert_eq!(status, 503),
        other => panic!("expected Upstream(503), got {other:?}"),
    }
}

#[tokio::test]
async fn anime_detail_returns_parse_failed_on_garbage_body() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(b"<html>not json</html>".to_vec()),
        )
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let r = client.anime_detail("12").await;
    assert!(matches!(
        r,
        Err(ani_gui::error::AniError::ParseFailed { .. })
    ));
}
