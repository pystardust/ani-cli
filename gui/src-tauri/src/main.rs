// Tauri entrypoint for ani-gui.
//
// Production builds suppress the console window on Windows; everywhere else
// this is a no-op.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() {
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

    // The actual Tauri builder wiring lives in lib::run() and is fleshed out
    // as commands and the streaming proxy come online. For now main exists
    // so the binary target builds.
    println!("ani-gui {}", ani_gui::VERSION);
}
