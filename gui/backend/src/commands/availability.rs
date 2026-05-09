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

/// Cache TTL for positive results on FINISHED shows — 30 days.
/// Catalog is stable; episode_count won't move.
const AVAILABILITY_TTL_FINISHED_SECS: u64 = 30 * 24 * 60 * 60;
/// Cache TTL for positive results on ONGOING shows (status =
/// "current" / "upcoming" / "tba") — 24 hours. Most shows release
/// once a week; a 1-day window means new episodes surface within
/// a day of the next probe rather than waiting up to a month for
/// the cap to refresh.
const AVAILABILITY_TTL_ONGOING_SECS: u64 = 24 * 60 * 60;
/// Cache TTL for negative results — 7 days. Catalog adds are rarer
/// than removals but still happen (late-season uploads, region
/// availability shifts), so refresh negatives more often.
const AVAILABILITY_TTL_NEGATIVE_SECS: u64 = 7 * 24 * 60 * 60;

/// Inputs for `availability_check` — a Kitsu title (plus alts) and
/// the audio mode under which to ask "is this on allmanga?". Carries
/// optional Kitsu metadata (`episode_count`, `status`, `kitsu_id`)
/// that lets the resolver disambiguate name collisions and pick the
/// right cache TTL bucket.
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
    /// Kitsu's airing status (`"current"`, `"finished"`,
    /// `"upcoming"`, `"tba"`, `"unreleased"`). When known, it
    /// branches the positive cache TTL: ongoing shows refresh in
    /// 24h so weekly drops surface within a day; finished shows
    /// hold for 30d. When omitted, defaults to the ongoing TTL —
    /// safer to refresh too often than to serve a stale cap.
    #[serde(default)]
    pub status: Option<String>,
}

/// Result of an availability probe — does allmanga carry the show in
/// the requested mode, and (when it does) what's the truthful episode
/// cap and recap-tag list?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityResponse {
    /// True when allmanga has at least one candidate matching any of
    /// the queried titles. False = the show is not in allmanga's
    /// catalog (e.g. Western animation Kitsu happens to index).
    pub available: bool,
    /// Highest INTEGER episode number streamable in the requested
    /// mode (One Piece sub: 1160). Authoritative cap for resume
    /// CTA, Download All / range max, episode-strip pagination.
    /// Excludes half-episode tags like `"1061.5"` — those live in
    /// `extra_episodes`. None when available=false or legacy cache.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Non-integer episode tags allmanga has streamable (recap /
    /// special episodes — e.g. `["1061.5"]` for One Piece).
    /// Frontend splices these into the episode strip at their
    /// numeric position. Empty when there are no extras.
    #[serde(default)]
    pub extra_episodes: Vec<String>,
}

/// Inputs for the batch `availability_cached` lookup — a list of
/// Kitsu ids and the mode to read cached results for. Skips the
/// network entirely; only returns entries that already have a value
/// in `meta_cache`.
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityBatchArgs {
    /// Kitsu ids to look up. Mode disambiguates sub/dub since a
    /// title may exist in one and not the other.
    pub kitsu_ids: Vec<String>,
    /// `"sub"` or `"dub"` — selects which cached probe to read.
    pub mode: String,
}

/// Cached-only batch response — `kitsu_id → available` for the rows
/// the cache had a value for. Missing ids are absent from the map and
/// the caller should render them optimistically while the lazy probe
/// fills in.
#[derive(Debug, Clone, Serialize)]
pub struct AvailabilityBatchResponse {
    /// Map of kitsu_id → available. Only contains entries that have
    /// a cached value; missing ids should be treated as "unknown,
    /// render normally" by the caller.
    pub cached: HashMap<String, bool>,
}

fn cache_key(kitsu_id: &str, mode: &str) -> String {
    // v3: SHOW_GQL fix — availableEpisodesDetail is a scalar, not
    //     an object; the v2-rollout query subselected `{ sub dub }`
    //     and got empty lists back, so episode_count fell through
    //     to the buggy COUNT path. Bumping again so v2 rows
    //     (already-poisoned with the count) get superseded.
    // v2: episode_count switched from "len of availableEpisodes list"
    //     to "max integer episode" via fetch_show.
    let m = if mode == "dub" { "dub" } else { "sub" };
    format!("availability:v3:{kitsu_id}:{m}")
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
    // Legacy rows (available=true, episode_count=None) are treated
    // as misses so they self-heal: the next visit re-probes and
    // populates the count. False rows are kept as-is — episode_count
    // is meaningless when there's no candidate.
    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        let key = cache_key(id, mode);
        if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
            if let Ok(parsed) = serde_json::from_str::<AvailabilityResponse>(&body) {
                let needs_refresh = parsed.available && parsed.episode_count.is_none();
                if !needs_refresh {
                    return Ok(parsed);
                }
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

    // For the cap we need the actual episode-tag list (allmanga's
    // `availableEpisodes` is a COUNT that includes half-episodes,
    // which makes it +1 too high for shows with recaps like One
    // Piece). Fetch the show's `availableEpisodesDetail` and split
    // into max-integer + non-integer extras. Failures fall back to
    // the count, which is wrong by ±1 in rare cases but better
    // than blocking the cache write.
    let mut episode_count: Option<u32> = None;
    let mut extra_episodes: Vec<String> = Vec::new();
    if let Some(c) = chosen_candidate.as_ref() {
        match crate::scraper::allanime::fetch_show(&state.proxy_http, &c.id, None).await {
            Ok(detail) => {
                episode_count = detail.max_integer_episode(mode);
                extra_episodes = detail
                    .available_episodes_detail
                    .for_mode(mode)
                    .iter()
                    .filter(|t| t.parse::<u32>().is_err())
                    .cloned()
                    .collect();
            }
            Err(_) => {
                // Show fetch failed — fall back to the count from the
                // search hit. Off by one for shows with halves, but
                // good enough for the cap until next probe.
                let n = c.available_episodes.for_mode(mode);
                if n > 0 {
                    episode_count = Some(n);
                }
            }
        }
    }

    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        write_cache_full(
            state,
            id,
            mode,
            available,
            episode_count,
            extra_episodes.clone(),
            args.status.as_deref(),
        );
    }

    Ok(AvailabilityResponse {
        available,
        episode_count,
        extra_episodes,
    })
}

/// Persist a known availability result. Public so the play and
/// download paths can update the cache from their own success /
/// NoResults outcomes — clicks from any tile end up populating the
/// cache without an extra network round-trip. Episode count is
/// unknown from these call sites (they don't read availableEpisodes
/// off the candidate), so the cache row stores None there; the next
/// detail-page probe will fill it in.
pub fn write_cache(state: &AppState, kitsu_id: &str, mode: &str, available: bool) {
    // Status unknown at this call site (play / download success /
    // failure paths). Use ongoing TTL — the next detail-page probe
    // will overwrite this row anyway, since check_availability's
    // self-heal kicks in for rows with episode_count=None.
    write_cache_full(state, kitsu_id, mode, available, None, Vec::new(), None);
}

/// Pick the positive cache TTL based on Kitsu's airing status.
/// Returns the short (24h) TTL for shows likely to get new episodes
/// soon, and the long (30d) TTL only for definitively finished
/// shows. Unknown status falls back to the short TTL — better to
/// re-probe than to serve a stale cap.
fn positive_ttl_for(status: Option<&str>) -> u64 {
    match status {
        Some("finished") => AVAILABILITY_TTL_FINISHED_SECS,
        _ => AVAILABILITY_TTL_ONGOING_SECS,
    }
}

/// Same as [`write_cache`] but lets the caller supply the episode
/// count + extras when it knows. Used by `check_availability` after
/// running the play picker; everything else stays on the simpler
/// entry point.
pub fn write_cache_full(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
    available: bool,
    episode_count: Option<u32>,
    extra_episodes: Vec<String>,
    status: Option<&str>,
) {
    if kitsu_id.is_empty() {
        return;
    }
    let key = cache_key(kitsu_id, mode);
    let ttl = if available {
        positive_ttl_for(status)
    } else {
        AVAILABILITY_TTL_NEGATIVE_SECS
    };
    let body = AvailabilityResponse {
        available,
        episode_count,
        extra_episodes,
    };
    if let Ok(body) = serde_json::to_string(&body) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ttl);
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

/// Inputs for `availability_warm` — a batch of titles whose cache
/// rows the backend should probe (and populate) in the background.
/// Drives the home-page hybrid filter that skips already-cached rows
/// and rate-limits the rest at one probe per 500 ms.
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityWarmArgs {
    /// Per-title args — same shape as the single-item check.
    pub items: Vec<AvailabilityArgs>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache body is the AvailabilityResponse JSON. Adding the new
    /// episode_count field must round-trip — the detail page reads it
    /// to size the episode list and gate the Download All / range cap.
    #[test]
    fn response_round_trips_with_episode_count_and_extras() {
        let r = AvailabilityResponse {
            available: true,
            episode_count: Some(1160),
            extra_episodes: vec!["1061.5".into()],
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: AvailabilityResponse = serde_json::from_str(&json).expect("deserialize");
        assert!(back.available);
        assert_eq!(back.episode_count, Some(1160));
        assert_eq!(back.extra_episodes, vec!["1061.5".to_string()]);
    }

    /// Old cache rows (written before the new fields existed) must
    /// still parse — TTL hasn't expired them yet, so during the
    /// rollout window we'll see them. They yield episode_count=None
    /// and extra_episodes=[], which the frontend treats as "fall back
    /// to Kitsu's count, no recap tiles".
    #[test]
    fn legacy_response_parses_with_defaults() {
        let legacy = r#"{"available":true}"#;
        let r: AvailabilityResponse = serde_json::from_str(legacy).expect("legacy parses");
        assert!(r.available);
        assert_eq!(r.episode_count, None);
        assert!(r.extra_episodes.is_empty());
    }

    /// `positive_ttl_for` is the load-bearing branch on Kitsu's
    /// status string. Misclassifying a finished show as ongoing only
    /// wastes a network probe; misclassifying an ongoing show as
    /// finished would hide weekly drops for 30 days. Both directions
    /// must be guarded.
    #[test]
    fn positive_ttl_for_finished_returns_30_day_window() {
        assert_eq!(positive_ttl_for(Some("finished")), 30 * 24 * 60 * 60);
    }

    #[test]
    fn positive_ttl_for_current_returns_24_hour_window() {
        // Kitsu's "current" maps to ongoing — the 24h window keeps
        // weekly drops visible within a day of upload.
        assert_eq!(positive_ttl_for(Some("current")), 24 * 60 * 60);
    }

    #[test]
    fn positive_ttl_for_unknown_or_missing_falls_back_to_ongoing() {
        // The "unknown status" branch has to favour ongoing — a
        // 30-day cap on a maybe-airing show is the worst failure
        // mode (silently stale episode count).
        for s in [
            None,
            Some("upcoming"),
            Some("tba"),
            Some("unreleased"),
            Some(""),
        ] {
            assert_eq!(
                positive_ttl_for(s),
                24 * 60 * 60,
                "expected ongoing TTL for status {s:?}"
            );
        }
    }
}
