//! "Is this title playable?" probe — runs the same allmanga search
//! the play path does, returns true iff any candidate exists for the
//! canonical title or any alt-title.
//!
//! The detail page hits this on mount so it can gate the Play +
//! Download CTAs ahead of a click — better than letting the user
//! discover "this show isn't on allmanga" by clicking and getting an
//! error overlay (the prior failure mode for shows like Kitsu's
//! Western-animation entries: "Arcane Season 2", etc.).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::play::{pick_title_and_index, PlayArgs};
use crate::error::Result;

/// Cache TTL for availability results — 7 days. allmanga's catalog
/// changes slowly, and a stale "available" hit just adds one wasted
/// click; a stale "unavailable" still self-heals when the cache
/// expires and the user revisits a detail page.
const AVAILABILITY_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityArgs {
    /// Canonical Kitsu title — first search target.
    pub title: String,
    /// `"sub"` or `"dub"` — gates the kind of result allmanga returns.
    pub mode: String,
    /// Fallback titles to try when canonical returns no hits (e.g.
    /// romanized + native name pulled from Kitsu's `titles.*`).
    #[serde(default)]
    pub alt_titles: Vec<String>,
    /// Kitsu's authoritative episode count — feeds the same picker
    /// disambiguation the play path uses (Stone Ocean Part 6 etc.).
    /// Without it, the picker falls back to first-hit, which means
    /// availability says "yes" for any show with a colliding name
    /// even when the actual show isn't on allmanga.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Kitsu id — cache key. When omitted (legacy callers), the
    /// check still runs but its result isn't persisted.
    #[serde(default)]
    pub kitsu_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityResponse {
    /// True when allmanga has at least one candidate matching any of
    /// the queried titles. False = the show is not in allmanga's
    /// catalog (e.g. Western animation Kitsu happens to index).
    pub available: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityBatchArgs {
    /// Kitsu ids to look up. Mode disambiguates sub/dub since a
    /// title may exist in one and not the other.
    pub kitsu_ids: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailabilityBatchResponse {
    /// Map of kitsu_id → available. Only contains entries that have
    /// a cached value; missing ids should be treated as "unknown,
    /// render normally" by the caller.
    pub cached: HashMap<String, bool>,
}

fn cache_key(kitsu_id: &str, mode: &str) -> String {
    let m = if mode == "dub" { "dub" } else { "sub" };
    format!("availability:{kitsu_id}:{m}")
}

/// Reuses the play path's `pick_title_and_index` so the cache
/// reflects the SAME disambiguation play does. Returns `available:
/// true` only when the picker lands on a specific candidate (shows
/// with name collisions but no Kitsu episode_count to disambiguate
/// fall back to first-hit; the picker still returns it via the
/// chosen Option, so available stays true — but at least the
/// signal matches what play would do at the same time).
///
/// # Errors
/// - Network errors from the underlying allmanga search propagate;
///   the frontend can fall back to the lazy click path.
pub async fn check_availability(
    state: &AppState,
    args: &AvailabilityArgs,
) -> Result<AvailabilityResponse> {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };

    // Cache short-circuit. Skipped when no kitsu_id is supplied.
    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        let key = cache_key(id, mode);
        if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
            if let Ok(parsed) = serde_json::from_str::<AvailabilityResponse>(&body) {
                return Ok(parsed);
            }
        }
    }

    // Funnel through the play picker so availability honours the
    // same disambiguation play uses. Synthesise a PlayArgs view —
    // episode + quality + prefetch are unused by pick_title_and_index
    // but the type needs them.
    let play_view = PlayArgs {
        title: args.title.clone(),
        episode: "1".into(),
        mode: mode.into(),
        quality: None,
        episode_count: args.episode_count,
        alt_titles: args.alt_titles.clone(),
        prefetch: false,
        kitsu_id: args.kitsu_id.clone(),
    };
    let (_chosen_title, _select, chosen_candidate) = pick_title_and_index(state, &play_view).await;
    let available = chosen_candidate.is_some();

    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        write_cache(state, id, mode, available);
    }

    Ok(AvailabilityResponse { available })
}

/// Persist a known availability result. Public so the play and
/// download paths can update the cache from their own success /
/// NoResults outcomes — clicks from any tile end up populating the
/// cache without an extra network round-trip.
pub fn write_cache(state: &AppState, kitsu_id: &str, mode: &str, available: bool) {
    if kitsu_id.is_empty() {
        return;
    }
    let key = cache_key(kitsu_id, mode);
    if let Ok(body) = serde_json::to_string(&AvailabilityResponse { available }) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, AVAILABILITY_TTL_SECS);
    }
}

/// Cached-only batch lookup. Returns `cached[id] = available` for
/// every id with a fresh cache entry; missing entries are absent
/// from the map so the caller can render them while waiting.
pub fn batch_cached(state: &AppState, args: &AvailabilityBatchArgs) -> AvailabilityBatchResponse {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    let mut cached = HashMap::with_capacity(args.kitsu_ids.len());
    for id in &args.kitsu_ids {
        if id.is_empty() {
            continue;
        }
        let key = cache_key(id, mode);
        if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
            if let Ok(parsed) = serde_json::from_str::<AvailabilityResponse>(&body) {
                cached.insert(id.clone(), parsed.available);
            }
        }
    }
    AvailabilityBatchResponse { cached }
}

/// Warm the cache for a set of titles. Each entry carries the data
/// needed to run check_availability (title + alt_titles + mode +
/// kitsu_id). Entries with an existing fresh cache entry are
/// skipped; the rest are probed sequentially with a 500ms gap
/// between queries so we don't hammer allmanga.
///
/// Designed to be called fire-and-forget after a list view renders;
/// the caller doesn't wait for the result. The next visit to the
/// same list reads the now-populated cache and filters known-
/// unavailable cards.
pub async fn warm(state: std::sync::Arc<AppState>, items: Vec<AvailabilityArgs>) {
    use tokio::time::{sleep, Duration};
    for args in items {
        let mode = if args.mode == "dub" { "dub" } else { "sub" };
        let id = match args.kitsu_id.as_deref() {
            Some(id) if !id.is_empty() => id,
            _ => continue,
        };
        // Skip entries that already have a fresh cache value.
        let key = cache_key(id, mode);
        if let Ok(Some(_)) = meta_cache_get(&state.cache_pool, &key) {
            continue;
        }
        let _ = check_availability(&state, &args).await;
        sleep(Duration::from_millis(500)).await;
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityWarmArgs {
    /// Per-title args — same shape as the single-item check.
    pub items: Vec<AvailabilityArgs>,
}
