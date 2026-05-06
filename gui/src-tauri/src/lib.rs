//! ani-gui — desktop GUI for the ani-cli anime scraper.
//!
//! This crate is the Rust backend of the Tauri 2 application. It does three
//! things on the user's machine:
//!
//! 1. Drives the vendored `ani-cli` script (subprocess) to scrape allanime
//!    for search results, episode lists, and resolved stream URLs.
//! 2. Runs a localhost streaming proxy that injects the `Referer:` header
//!    every CDN requires and rewrites m3u8 manifests so the embedded
//!    `<video>` + `hls.js` player can fetch segments without CORS pain.
//! 3. Talks to Kitsu and AniList for metadata, caches results in SQLite +
//!    images on disk, and reads the shared ani-cli history file.
//!
//! No internet-reachable services are ever started — every listening socket
//! is bound to `127.0.0.1`. The crate's organizing principle is that the
//! frontend (SvelteKit, runs inside the Tauri webview) only ever talks to
//! Tauri commands and to the localhost proxy.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod anicli;
pub mod app;
pub mod cache;
pub mod commands;
pub mod config;
pub mod error;
pub mod history;
pub mod i18n;
pub mod meta;
pub mod proxy;

pub use error::{AniError, Result};

/// Library version, sourced from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Boot the desktop app: bind the streaming proxy on a kernel-assigned
/// loopback port, wire AppState into Tauri's managed state, register the
/// command handlers, and run the event loop.
///
/// Called from `main.rs`. Blocks until the window closes.
///
/// # Errors
/// Returns [`AniError`] when the proxy listener can't bind, ani-cli
/// can't be located, or Tauri's builder fails.
pub fn run() -> Result<()> {
    use std::sync::Arc;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| AniError::Io)?;
    let runtime = Arc::new(runtime);

    // Build the proxy listener + http client + AppState on the runtime.
    let app_state = runtime.block_on(async {
        let proxy_http = proxy::upstream::build_client()?;
        let (addr, listener) = proxy::bind_loopback(0).await?;
        let origin = proxy::ProxyOrigin::new(&addr.ip().to_string(), addr.port());
        let state = app::AppState::build(proxy_http, origin, None)?;

        // Spawn the axum proxy router with a clone of state's proxy view.
        let router = proxy::build_router(state.proxy_state());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "stream proxy server stopped");
            }
        });
        tracing::info!(
            proxy = %state.proxy_origin.base,
            ani_cli = %state.ani_cli_path.display(),
            history = %state.history_path.display(),
            "ani-gui backend ready"
        );
        Ok::<_, AniError>(state)
    })?;

    // Hand the runtime to Tauri so #[tauri::command] async fns run on it.
    let runtime_for_tauri = runtime.clone();

    let runtime_for_protocol = runtime.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .register_asynchronous_uri_scheme_protocol("image", move |ctx, request, responder| {
            use tauri::http::Response as HttpResponse;
            use tauri::Manager;

            // Pull the live state + http client from Tauri's managed state,
            // hand the actual fetch/cache work to our tokio runtime so the
            // webview thread isn't blocked on disk + network.
            let handle = ctx.app_handle();
            let state = handle.state::<app::AppState>();
            let client = state.proxy_http.clone();
            let cache_dir = state.image_cache_dir.clone();
            let runtime = runtime_for_protocol.clone();
            let uri = request.uri().to_string();
            runtime.spawn(async move {
                let response: HttpResponse<Vec<u8>> =
                    match meta::images::handle_protocol_request(&client, &cache_dir, &uri).await {
                        Ok((bytes, mime)) => HttpResponse::builder()
                            .status(200)
                            .header("content-type", mime)
                            // 24h: poster URLs are stable; the URL itself
                            // changes when the upstream image changes.
                            .header("cache-control", "public, max-age=86400")
                            .body(bytes)
                            .unwrap_or_else(|_| HttpResponse::new(Vec::new())),
                        Err(e) => {
                            tracing::warn!(uri = %uri, error = ?e, "image:// protocol miss");
                            HttpResponse::builder()
                                .status(502)
                                .body(Vec::new())
                                .unwrap_or_else(|_| HttpResponse::new(Vec::new()))
                        }
                    };
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::ipc::cmd_app_info,
            commands::ipc::cmd_proxy_base_url,
            commands::ipc::cmd_history_list,
            commands::ipc::cmd_history_clear,
            commands::ipc::cmd_open_external_player,
            commands::ipc::cmd_create_session,
            commands::ipc::cmd_kitsu_search,
            commands::ipc::cmd_kitsu_anime_detail,
            commands::ipc::cmd_kitsu_trending,
            commands::ipc::cmd_kitsu_top_rated,
            commands::ipc::cmd_kitsu_episodes,
            commands::ipc::cmd_title_match_get,
            commands::ipc::cmd_title_match_put,
            commands::ipc::cmd_settings_get,
            commands::ipc::cmd_settings_put,
        ])
        .setup(move |app| {
            // Tauri 2 owns its own runtime; we keep ours alive for the
            // proxy task by leaking the Arc into the app's data.
            let _ = app;
            let _keep = runtime_for_tauri.clone();
            std::mem::forget(_keep);
            Ok(())
        })
        .run(tauri::generate_context!())
        .map_err(|e| {
            tracing::error!(error = %e, "tauri builder failed");
            AniError::Io
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_looks_like_semver() {
        // env!() produces a `&'static str`; clippy is right that
        // is_empty() on a const is silly. The check that matters is shape.
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(
            parts.len() >= 2,
            "CARGO_PKG_VERSION should be semver-shaped: {VERSION}"
        );
    }
}
