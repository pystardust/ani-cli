//! Drift detector for `ani-cli`'s progress lines.
//!
//! The SSE loading overlay (M2g) parses ani-cli's stderr line-by-line
//! and forwards `<provider> Links Fetched` to the renderer so the user
//! sees what step we're on. The format comes from a single
//! `printf "\033[1;32m%s\033[0m Links Fetched\n"` inside ani-cli's
//! `provider_init` (line ~168 of the script). We don't own that
//! script — `pystardust/ani-cli` does — and they patch its scrape
//! every few weeks.
//!
//! This test runs the **real** vendored ani-cli through the curl shim
//! and asserts the stderr still contains the expected format. If
//! upstream renames a provider, drops the message, or changes the
//! suffix, this test fails loudly — long before users see "Loading…"
//! with no progress text.
//!
//! Linux-only for the same reason as `anicli_run_debug.rs`.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use ani_gui::anicli::parser::{parse_progress_line, strip_ansi, ProgressLine};

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

#[test]
fn ani_cli_emits_links_fetched_progress_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let fixtures = tmp.path().join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("mkdir fixtures");
    build_fixtures(&fixtures);
    let bin = stage_curl_shim_wrapper(tmp.path(), &fixtures);
    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    let ani_cli_path = repo_root().join("ani-cli");
    assert!(ani_cli_path.is_file(), "ani-cli script exists");

    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{system_path}", bin.display());

    // Use the real script via std::process::Command so we can capture
    // stderr verbatim. The Rust driver intentionally splits stdout /
    // stderr, but here we want to assert on stderr directly because
    // that's where progress messages flow.
    let output = Command::new(&ani_cli_path)
        .args(["-S", "1", "-e", "1", "-q", "best", "--", "test"])
        .env_clear()
        .env("PATH", &path)
        .env("HOME", tmp.path())
        .env("ANI_CLI_HIST_DIR", &hist)
        .env("ANI_CLI_PLAYER", "debug")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .output()
        .expect("ani-cli runs");

    assert!(
        output.status.success(),
        "ani-cli exited with status {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = strip_ansi(&output.stderr);
    let parsed: Vec<ProgressLine> = stderr.lines().filter_map(parse_progress_line).collect();

    // Drift contract: at least one `<provider> Links Fetched` line
    // must show up. Which providers fire depends on the fixture, but
    // the format itself is the load-bearing assertion. If upstream
    // renames the suffix or drops it, this fails — and someone needs
    // to update parse_progress_line() before the SSE overlay regresses
    // to no-text.
    let has_links_fetched = parsed
        .iter()
        .any(|p| matches!(p, ProgressLine::LinksFetched { .. }));
    assert!(
        has_links_fetched,
        "ani-cli stderr no longer contains a `<provider> Links Fetched` line. \
         Either upstream changed the format (update parse_progress_line) or our \
         fixture stopped triggering provider_init. Captured lines:\n{parsed:#?}"
    );
}
