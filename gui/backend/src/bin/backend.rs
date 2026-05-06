//! Standalone backend binary — same Rust logic as the Tauri-bundled
//! app, minus Tauri itself. Used by Electron's main process, which
//! spawns this as a sidecar and reads its stdout to learn the bound
//! port.
//!
//! Stdout protocol: a single line of the form
//!     ANI_GUI_LISTENING http://127.0.0.1:<port>
//! is printed once the axum server is accepting connections. The
//! Electron main process matches that prefix, parses the URL, and
//! injects it into the renderer via the preload script's
//! `window.aniGui.apiBase`.
//!
//! After printing, this binary blocks on the axum server forever.
//! Electron sends SIGTERM on app quit; we have no in-process
//! shutdown channel — the OS reaps us cleanly because all threads
//! are tokio's, none owning external resources beyond the SQLite
//! pool (closed on drop) and reqwest sockets (closed on drop).

#![forbid(unsafe_code)]

use std::sync::Arc;

use ani_gui::{api, app, proxy, AniError};

fn main() -> std::process::ExitCode {
    // Logging — RUST_LOG honoured, default keeps the noise down.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "ani_gui=info".into());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(true)
        .compact()
        .init();

    tracing::info!(version = ani_gui::VERSION, "starting ani-gui-backend");

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => Arc::new(r),
        Err(e) => {
            tracing::error!(error = %e, "tokio runtime build failed");
            return std::process::ExitCode::FAILURE;
        }
    };

    // Bind, build state, spawn server, hold the runtime.
    let result = runtime.block_on(async {
        let proxy_http = proxy::upstream::build_client()?;
        let (addr, listener) = proxy::bind_loopback(0).await?;
        let origin = proxy::ProxyOrigin::new(&addr.ip().to_string(), addr.port());
        let state = app::AppState::build(proxy_http, origin.clone(), None)?;

        let proxy_router = proxy::build_router(state.proxy_state());
        let api_router = api::build_api_router(Arc::new(state));
        let router = proxy_router.merge(api_router);

        // The handshake: print the URL the Electron main process is
        // waiting for, then run forever. Flushing stdout matters —
        // Electron may buffer line-by-line, so any partial line could
        // hang the spawn.
        println!("ANI_GUI_LISTENING {}", origin.base);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        tracing::info!(addr = %addr, "ani-gui-backend ready");

        axum::serve(listener, router)
            .await
            .map_err(|_| AniError::Network)?;
        Ok::<_, AniError>(())
    });

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = ?e, "ani-gui-backend exited with error");
            std::process::ExitCode::FAILURE
        }
    }
}
