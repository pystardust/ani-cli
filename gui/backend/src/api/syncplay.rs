//! Axum handler for the `/api/play/syncplay` route.
//!
//! Lives in its own submodule (not inline in `api/mod.rs`) so the
//! parent module's aggregate ccn doesn't tick up every time a new
//! play-style terminal lands. The handler itself is a thin wrapper
//! around `commands::syncplay::play_syncplay` — the resolution
//! chain, cache reuse, and spawn logic all live in
//! `commands::syncplay`.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::app::AppState;
use crate::commands::{play::PlayArgs, play_syncplay};
use crate::error::AniError;

/// Sister of `post_play_external` — same resolution chain, but the
/// resolved URL goes to the user's Syncplay binary instead of mpv.
/// Returns 202 Accepted because Syncplay launches in a detached
/// process and we don't wait for it. A failed spawn surfaces as
/// `AniError::SyncplaySpawnFailed { binary }`; the frontend's
/// ErrorOverlay names the binary and links to syncplay.pl.
pub(super) async fn post_play_syncplay(
    State(state): State<Arc<AppState>>,
    Json(args): Json<PlayArgs>,
) -> Result<StatusCode, AniError> {
    play_syncplay::play_syncplay(&state, &args).await?;
    Ok(StatusCode::ACCEPTED)
}
