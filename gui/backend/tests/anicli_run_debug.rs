//! End-to-end integration test for [`ani_gui::anicli::process::run_debug`].
//!
//! Spawns the real `ani-cli` script with a `curl` shim placed on PATH that
//! returns canned fixtures (the same shim used by `tests/bash/acceptance/`).
//! Verifies that the Rust driver:
//!
//! 1. Spawns the script with the right argv + env scrubbing.
//! 2. Reads stdout, strips ANSI, parses the `Selected link:` block.
//! 3. Returns the resolved URL via `DebugOutput`.
//!
//! Linux-only: `ani-cli` is a POSIX-shell script that depends on bash + a
//! POSIX environment. macOS bash is too old in places to be reliable, and
//! Windows has no native bash at all. The Rust driver is portable; this
//! particular integration test isn't.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use ani_gui::anicli::process::{run_debug, DebugOptions};

/// Repo root, computed from this test file's location.
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest is two levels deep from repo root")
        .to_path_buf()
}

/// Build a fixture directory matching the layout the curl shim expects.
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
    // Run blob_builder.sh to synthesize the encrypted episode_blob.json.
    let status = Command::new("bash")
        .arg(root.join("tests/bash/helpers/blob_builder.sh"))
        .arg(dir.join("episode_blob.json"))
        .status()
        .expect("blob_builder.sh runs");
    assert!(status.success(), "blob_builder.sh exited 0");
}

/// Stage the curl shim as a `curl` executable on a fresh tmp dir, return
/// that dir so it can be prepended to PATH.
fn stage_curl_shim(tmp: &std::path::Path) -> PathBuf {
    let bin = tmp.join("bin");
    std::fs::create_dir_all(&bin).expect("mkdir bin");
    let shim_src = repo_root().join("tests/bash/helpers/curl_shim.sh");
    let shim_dst = bin.join("curl");
    std::fs::copy(&shim_src, &shim_dst).expect("copy curl shim");
    // `mut` is only used in the cfg(unix) arm; allow(unused_mut) keeps
    // the Windows build clean under -D warnings.
    #[allow(unused_mut)]
    let mut perms = std::fs::metadata(&shim_dst).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&shim_dst, perms).expect("chmod +x");
    bin
}

#[tokio::test]
async fn run_debug_resolves_wixmp_url_via_curl_shim() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let fixtures = tmp.path().join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("mkdir fixtures");
    build_fixtures(&fixtures);

    let bin = stage_curl_shim(tmp.path());

    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    // The curl shim reads CURL_FIXTURE_DIR. tokio::process::Command env
    // propagates everything we set via .env(), so we set it from the
    // test process and rely on env inheritance for ani-cli's curl
    // invocations. (run_debug does env_clear() but propagates PATH and
    // HOME explicitly; the curl shim's CURL_FIXTURE_DIR needs to be
    // explicitly threaded — set via std::env so ani-cli inherits it
    // through the dispatcher's normal env chain... but env_clear()
    // breaks that. Instead, the shim falls back to a default if
    // CURL_FIXTURE_DIR is unset.)
    //
    // Workaround: write a tiny wrapper shim that sets the env for us.
    let wrapped_shim = bin.join("curl");
    let shim_body = format!(
        "#!/bin/sh\nexport CURL_FIXTURE_DIR={fixtures}\nexec sh {repo}/tests/bash/helpers/curl_shim.sh \"$@\"\n",
        fixtures = fixtures.display(),
        repo = repo_root().display(),
    );
    std::fs::write(&wrapped_shim, shim_body).expect("write wrapped shim");
    #[allow(unused_mut)]
    let mut perms = std::fs::metadata(&wrapped_shim).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&wrapped_shim, perms).expect("chmod +x");

    // Locate ani-cli at the repo root.
    let ani_cli_path = repo_root().join("ani-cli");
    assert!(ani_cli_path.is_file(), "ani-cli script exists");

    // Compose PATH: tmp/bin (with our curl shim) ahead of the system
    // path so the script's curl resolves to ours.
    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{system_path}", bin.display());

    let opts = DebugOptions {
        ani_cli_path,
        hist_dir: Some(hist),
        timeout: std::time::Duration::from_secs(60),
        path_override: Some(path),
    };

    let out = match run_debug(&opts, "test", "1", "best", "sub").await {
        Ok(v) => v,
        Err(e) => {
            // CI has been failing here with `error.scraper.parse_failed`
            // and the bare assertion gave us nothing to debug from. On
            // failure, dump every input the test process can still
            // observe so the next CI run carries enough breadcrumbs.
            let dump_dir = |label: &str, p: &std::path::Path| {
                eprintln!("--- {label}: {} ---", p.display());
                if let Ok(entries) = std::fs::read_dir(p) {
                    for e in entries.flatten() {
                        eprintln!("  {}", e.path().display());
                    }
                }
            };
            eprintln!("\n=== run_debug failed: {e:?} ===");
            dump_dir("tmp", tmp.path());
            dump_dir("fixtures", &fixtures);
            dump_dir("bin", &bin);
            eprintln!("--- ani-cli script: {} ---", opts.ani_cli_path.display());
            eprintln!("--- PATH override: {:?} ---", opts.path_override);
            eprintln!("--- repo_root() = {} ---", repo_root().display());

            // Probe ani-cli's `dep_ch` deps directly. The previous
            // diagnostic dump told us env state was correct, but
            // `run_debug` consumed the script's actual stdout/stderr
            // before returning the parse-failed error key. Rerun with
            // `-h` (which exercises dep_ch then exits immediately) and
            // fall back to a `command -v` probe for each dep so we can
            // tell at a glance whether the runner is missing fzf,
            // openssl, etc. — that's the most likely failure mode.
            let path_str = opts.path_override.clone().unwrap_or_default();
            let probe = std::process::Command::new(&opts.ani_cli_path)
                .env_clear()
                .env("PATH", &path_str)
                .env("HOME", "/tmp")
                .arg("-h")
                .output();
            match probe {
                Ok(o) => {
                    eprintln!("--- ani-cli -h exit: {:?} ---", o.status);
                    eprintln!(
                        "--- ani-cli -h stdout ---\n{}",
                        String::from_utf8_lossy(&o.stdout)
                    );
                    eprintln!(
                        "--- ani-cli -h stderr ---\n{}",
                        String::from_utf8_lossy(&o.stderr)
                    );
                }
                Err(spawn_err) => {
                    eprintln!("--- ani-cli -h spawn failed: {spawn_err} ---");
                }
            }
            for tool in [
                "fzf", "curl", "openssl", "sed", "grep", "mpv", "aria2c", "ffmpeg",
            ] {
                let r = std::process::Command::new("sh")
                    .env_clear()
                    .env("PATH", &path_str)
                    .arg("-c")
                    .arg(format!("command -v {tool} || echo MISSING"))
                    .output();
                if let Ok(o) = r {
                    eprintln!(
                        "--- which {tool}: {} ---",
                        String::from_utf8_lossy(&o.stdout).trim()
                    );
                }
            }
            panic!("run_debug failed; see stderr dump above for env state");
        }
    };

    assert_eq!(out.selected_url, "https://wixmp.example/video.mp4");
    assert!(out
        .all_links
        .iter()
        .any(|l| l == "720 >https://wixmp.example/video.mp4"));
}

/// `run_search` is intentionally a stub today (see
/// `.planning/cli-contract-deviations.md`). This test pins the contract so
/// a future implementation accidentally not adding tests is loud.
#[tokio::test]
async fn run_search_returns_empty_until_unblocked() {
    let v = ani_gui::anicli::process::run_search("anything", "sub")
        .await
        .expect("stub returns Ok");
    assert!(v.is_empty(), "stub yields no results");
}
