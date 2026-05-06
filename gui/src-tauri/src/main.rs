// Tauri entrypoint for ani-gui.
//
// Production builds suppress the console window on Windows; everywhere else
// this is a no-op.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() {
    // webkit2gtk perf knobs — set BEFORE Tauri/WebKit initialise, or
    // they're ignored. These force GPU-backed compositing on Linux,
    // which on some distros (Arch, certain Debian builds) is
    // disabled by default and silently falls back to a software
    // rasterizer that can't keep 60 fps on widescreen scroll.
    //
    // - WEBKIT_DISABLE_COMPOSITING_MODE=0: undo any system-wide
    //   override that turned compositing off.
    // - WEBKIT_FORCE_COMPOSITING_MODE=1: explicitly enable; takes
    //   effect even when the heuristic that auto-detects "should we
    //   composite this page?" decides no.
    // - WEBKIT_USE_DMABUF_RENDERER=1: route compositor surfaces
    //   through DMA-BUF for zero-copy GPU sharing. Requires
    //   webkit2gtk >= 2.42 and a working EGL stack; falls back
    //   transparently when unavailable.
    //
    // Only set when not already configured so a power user can
    // override on the command line if they're debugging.
    #[cfg(target_os = "linux")]
    {
        for (k, v) in [
            ("WEBKIT_DISABLE_COMPOSITING_MODE", "0"),
            ("WEBKIT_FORCE_COMPOSITING_MODE", "1"),
            ("WEBKIT_USE_DMABUF_RENDERER", "1"),
        ] {
            if std::env::var_os(k).is_none() {
                // SAFETY: set_var on the host process before any
                // threads are spawned. Tauri's async runtime starts
                // later in run(); we're still single-threaded here.
                unsafe {
                    std::env::set_var(k, v);
                }
            }
        }
    }

    // Initialize structured logging. Honors `RUST_LOG`, defaults to
    // `ani_gui=info,tauri=warn` so the frontend HMR loop isn't drowned in
    // noise.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "ani_gui=info,tauri=warn".into());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(true)
        .compact()
        .init();

    tracing::info!(version = ani_gui::VERSION, "starting ani-gui");

    if let Err(e) = ani_gui::run() {
        tracing::error!(error = ?e, "ani-gui exited with error");
        std::process::exit(1);
    }
}
