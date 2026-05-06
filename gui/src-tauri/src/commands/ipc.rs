//! Tauri `#[tauri::command]` wrappers around the plain command bodies.
//!
//! These adapter functions are the only place in the crate that uses
//! `tauri::State`, so the underlying logic in sibling modules
//! (`app_info`, `proxy_url`, `history`, `external_player`) stays
//! framework-free and unit-testable without booting Tauri.

use tauri::State;

use crate::app::AppState;
use crate::commands::{
    app_info, external_player, history as h_inner, kitsu as kitsu_inner,
    proxy_url::proxy_base_url as inner_proxy, session as session_inner, settings as settings_inner,
};
use crate::config::Config;
use crate::error::Result;
use crate::history::HistoryEntry;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuEpisode};

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

/// Frontend → backend: search Kitsu for anime by free-text. Cached.
#[tauri::command]
pub async fn cmd_kitsu_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<KitsuAnimeRef>> {
    kitsu_inner::kitsu_search(&state, &query).await
}

/// Frontend → backend: fetch a single anime detail by Kitsu id. Cached.
#[tauri::command]
pub async fn cmd_kitsu_anime_detail(
    state: State<'_, AppState>,
    id: String,
) -> Result<KitsuAnimeRef> {
    kitsu_inner::kitsu_anime_detail(&state, &id).await
}

/// Frontend → backend: fetch a single anime by slug. Returns `None`
/// when no entry matches. Used as the title-match resolver's last-
/// resort fallback when Kitsu's text search drops a sequel.
#[tauri::command]
pub async fn cmd_kitsu_anime_by_slug(
    state: State<'_, AppState>,
    slug: String,
) -> Result<Option<KitsuAnimeRef>> {
    kitsu_inner::kitsu_anime_by_slug(&state, &slug).await
}

/// Frontend → backend: trending row (currently-airing, ranked by users).
#[tauri::command]
pub async fn cmd_kitsu_trending(state: State<'_, AppState>) -> Result<Vec<KitsuAnimeRef>> {
    kitsu_inner::kitsu_trending(&state).await
}

/// Frontend → backend: top-rated row.
#[tauri::command]
pub async fn cmd_kitsu_top_rated(state: State<'_, AppState>) -> Result<Vec<KitsuAnimeRef>> {
    kitsu_inner::kitsu_top_rated(&state).await
}

/// Frontend → backend: a page of episodes for an anime detail page.
/// `page` is 1-based; defaults to 1 if omitted.
#[tauri::command]
pub async fn cmd_kitsu_episodes(
    state: State<'_, AppState>,
    anime_id: String,
    page: Option<u32>,
) -> Result<Vec<KitsuEpisode>> {
    kitsu_inner::kitsu_episodes(&state, &anime_id, page.unwrap_or(1)).await
}

/// Frontend → backend: read the cached `(title, cour) → kitsu_id`
/// mapping for a Continue Watching row. Returns `None` on miss; caller
/// is expected to fall through to a fresh kitsuSearch + pickKitsuMatch
/// and re-`put` on success.
#[tauri::command]
pub fn cmd_title_match_get(
    state: State<'_, AppState>,
    title: String,
    cour: u32,
) -> Result<Option<String>> {
    kitsu_inner::title_match_get(&state, &title, cour)
}

/// Frontend → backend: persist a `(title, cour) → kitsu_id` mapping
/// under TITLE_MATCH_TTL (30d).
#[tauri::command]
pub fn cmd_title_match_put(
    state: State<'_, AppState>,
    title: String,
    cour: u32,
    kitsu_id: String,
) -> Result<()> {
    kitsu_inner::title_match_put(&state, &title, cour, &kitsu_id)
}

/// Frontend → backend: read user settings (defaults when file is absent).
#[tauri::command]
pub fn cmd_settings_get(state: State<'_, AppState>) -> Result<Config> {
    settings_inner::settings_get(&state)
}

/// Frontend → backend: wipe every row from the SQLite meta_cache
/// table — Kitsu search results, anime details, episode lists,
/// title-match mappings. Does NOT touch the ani-cli history TSV
/// (which lives at $XDG_STATE_HOME/ani-cli/ani-hsts and is read-only
/// to us anyway), the on-disk image cache, or settings.
///
/// Used by the diagnostics page when cached data goes stale (e.g. an
/// older app version persisted a wrong title→kitsu_id mapping that
/// the current resolution rules need to invalidate).
#[tauri::command]
pub fn cmd_meta_cache_clear(state: State<'_, AppState>) -> Result<()> {
    crate::cache::meta_cache_clear(&state.cache_pool)
}

/// Frontend → backend: write the full settings struct atomically.
#[tauri::command]
pub fn cmd_settings_put(state: State<'_, AppState>, cfg: Config) -> Result<()> {
    settings_inner::settings_put(&state, &cfg)
}
