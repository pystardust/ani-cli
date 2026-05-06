//! End-to-end integration test for the M2 play endpoints.
//!
//! Mirrors the curl-shim staging from `anicli_run_debug.rs` — stages
//! the vendored shim ahead of the system PATH, copies the canned
//! allanime fixtures into a tmp dir, then drives `POST /api/play`
//! through the full axum router. Verifies the chain
//!
//!   POST /api/play  →  run_debug spawns ani-cli (which calls our shim)
//!                  →  parse `Selected link:` from stdout
//!                  →  create_session wraps the upstream URL
//!                  →  return CreateSessionResponse
//!
//! works end-to-end. The unit tests in `api/mod.rs` cover the route's
//! body validation; this file covers the actual subprocess path.
//!
//! Linux-only for the same reason as `anicli_run_debug.rs` — `ani-cli`
//! depends on a POSIX shell + GNU userland.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use ani_gui::api::build_api_router;
use ani_gui::app::{AppState, SCRAPER_CONCURRENCY};
use ani_gui::cache;
use ani_gui::meta::kitsu::KitsuClient;
use ani_gui::proxy::{AppSecret, ProxyOrigin, SessionTable};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest is two levels deep from repo root")
        .to_path_buf()
}

fn build_fixtures(dir: &std::path::Path) {
    let root = repo_root();
    let src = root.join("tests/fixtures/allanime");
    for f in [
        "search_one_piece.json",
        "episodes_short.json",
        "embed_simple.json",
    ] {
        std::fs::copy(src.join(f), dir.join(f)).expect("copy fixture");
    }
    let status = Command::new("bash")
        .arg(root.join("tests/bash/helpers/blob_builder.sh"))
        .arg(dir.join("episode_blob.json"))
        .status()
        .expect("blob_builder.sh runs");
    assert!(status.success(), "blob_builder.sh exited 0");
}

/// Stage a wrapper `curl` script that re-execs the repo's curl shim
/// with the right `CURL_FIXTURE_DIR` set. Returns the bin/ dir to
/// prepend to PATH. Same construction as `anicli_run_debug.rs` —
/// kept inline rather than extracted because the two test files have
/// only this one piece in common and a shared helper would couple
/// them more than they're already coupled.
fn stage_curl_shim_wrapper(tmp: &std::path::Path, fixtures_dir: &std::path::Path) -> PathBuf {
    let bin = tmp.join("bin");
    std::fs::create_dir_all(&bin).expect("mkdir bin");
    let wrapped = bin.join("curl");
    let body = format!(
        "#!/bin/sh\nexport CURL_FIXTURE_DIR={fixtures}\nexec sh {repo}/tests/bash/helpers/curl_shim.sh \"$@\"\n",
        fixtures = fixtures_dir.display(),
        repo = repo_root().display(),
    );
    std::fs::write(&wrapped, body).expect("write wrapper shim");
    #[allow(unused_mut)]
    let mut perms = std::fs::metadata(&wrapped).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&wrapped, perms).expect("chmod +x");
    bin
}

/// Build an `AppState` pointed at the real `ani-cli` script and
/// otherwise pinned to the test's tmp dir. The test process's
/// `$PATH` should include the staged shim before this runs — the
/// play handler invokes `run_debug` with `path_override: None`, so
/// it inherits whatever PATH the test sets.
fn build_state(tmp: &std::path::Path) -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: repo_root().join("ani-cli"),
        history_path: tmp.join("hist/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
        image_cache_dir: tmp.join("images"),
        cache_pool: cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: tmp.join("config.toml"),
    }
}

#[tokio::test]
async fn play_endpoint_resolves_through_curl_shim_and_returns_session() {
    let tmp = TempDir::new().expect("tempdir");
    let fixtures = tmp.path().join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("mkdir fixtures");
    build_fixtures(&fixtures);
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");

    let bin = stage_curl_shim_wrapper(tmp.path(), &fixtures);

    // Prepend the shim dir to the test process's PATH. The play
    // handler's `run_debug` call inherits this PATH, so ani-cli's
    // `curl` invocations resolve to our shim. A previous PATH is
    // restored at the end of the test for hygiene — even though
    // each integration-test file runs in its own process, doing
    // so makes a future shared-test refactor safer.
    let saved_path = std::env::var("PATH").ok();
    let new_path = format!("{}:{}", bin.display(), saved_path.as_deref().unwrap_or(""));
    std::env::set_var("PATH", &new_path);

    let result = run_play_assertion(tmp.path()).await;

    if let Some(p) = saved_path {
        std::env::set_var("PATH", p);
    } else {
        std::env::remove_var("PATH");
    }
    result.expect("play assertion succeeded");
}

async fn run_play_assertion(tmp: &std::path::Path) -> Result<(), String> {
    let router = build_api_router(Arc::new(build_state(tmp)));
    let body = r#"{"title":"test","episode":"1","mode":"sub","quality":"best"}"#;
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("oneshot");

    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("collect body: {e}"))?
        .to_bytes();
    if status != StatusCode::OK {
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("expected 200, got {status}; body: {body_str}"));
    }
    // CreateSessionResponse is Serialize-only on purpose (the
    // backend produces it; nobody parses it on the Rust side).
    // Asserting via serde_json::Value keeps that contract intact.
    let resp: serde_json::Value =
        serde_json::from_slice(&body_bytes).map_err(|e| format!("parse body: {e}"))?;
    let media_url = resp
        .get("media_url")
        .and_then(|v| v.as_str())
        .ok_or("response missing media_url")?;
    let session_id = resp
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or("response missing session_id")?;
    let media_kind = resp
        .get("media_kind")
        .and_then(|v| v.as_str())
        .ok_or("response missing media_kind")?;
    // The shim resolves a wixmp MP4 (matching the fixture in
    // tests/fixtures/allanime/embed_simple.json), so the kind is mp4
    // and the proxy URL points at /file.mp4.
    if media_kind != "mp4" {
        return Err(format!("expected media_kind=mp4, got {media_kind}"));
    }
    if !media_url.contains("/s/") || !media_url.ends_with("/file.mp4") {
        return Err(format!("unexpected media_url shape: {media_url}"));
    }
    if session_id.is_empty() {
        return Err("session_id was empty".into());
    }
    Ok(())
}
