//! Tauri `#[tauri::command]` wrappers around the plain command bodies.
//!
//! These adapter functions are the only place in the crate that uses
//! `tauri::State`, so the underlying logic in sibling modules
//! (`app_info`, `proxy_url`, `history`, `external_player`) stays
//! framework-free and unit-testable without booting Tauri.

use tauri::State;

use crate::app::AppState;
use crate::commands::{
    app_info, external_player, history as h_inner, proxy_url::proxy_base_url as inner_proxy,
    session as session_inner,
};
use crate::error::Result;
use crate::history::HistoryEntry;

/// Frontend → backend: meta about the running backend.
#[tauri::command]
pub fn cmd_app_info(state: State<'_, AppState>) -> Result<app_info::AppInfo> {
    app_info::app_info(&state)
}

/// Frontend → backend: where the local stream proxy is listening.
#[tauri::command]
pub fn cmd_proxy_base_url(state: State<'_, AppState>) -> Result<String> {
    inner_proxy(&state)
}

/// Frontend → backend: the user's continue-watching list.
#[tauri::command]
pub fn cmd_history_list(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>> {
    h_inner::history_list(&state)
}

/// Frontend → backend: clear all history (mirrors `ani-cli -D`).
#[tauri::command]
pub fn cmd_history_clear(state: State<'_, AppState>) -> Result<()> {
    h_inner::history_clear(&state)
}

/// Frontend → backend: launch the external player escape hatch.
#[tauri::command]
pub fn cmd_open_external_player(args: external_player::LaunchArgs) -> Result<()> {
    external_player::open_external_player(&args)
}

/// Frontend → backend: register a stream session and get back the proxy
/// URL the embedded `<video>` / hls.js should load.
#[tauri::command]
pub fn cmd_create_session(
    state: State<'_, AppState>,
    args: session_inner::CreateSessionArgs,
) -> Result<session_inner::CreateSessionResponse> {
    session_inner::create_session(&state, &args)
}
