//! "Is this title playable?" probe — runs the same allmanga search
//! the play path does, returns true iff any candidate exists for the
//! canonical title or any alt-title.
//!
//! The detail page hits this on mount so it can gate the Play +
//! Download CTAs ahead of a click — better than letting the user
//! discover "this show isn't on allmanga" by clicking and getting an
//! error overlay (the prior failure mode for shows like Kitsu's
//! Western-animation entries: "Arcane Season 2", etc.).

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::Result;
use crate::scraper;

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
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailabilityResponse {
    /// True when allmanga has at least one candidate matching any of
    /// the queried titles. False = the show is not in allmanga's
    /// catalog (e.g. Western animation Kitsu happens to index).
    pub available: bool,
}

/// Walk title + alt_titles through allmanga's search; any non-empty
/// result returns `available: true`. Stops at the first hit so this
/// is fast for available shows; falls through up to ~3 queries before
/// returning `false`.
///
/// # Errors
/// - Network errors from `scraper::search` propagate (caller can
///   choose to fall back to the lazy error path).
pub async fn check_availability(
    state: &AppState,
    args: &AvailabilityArgs,
) -> Result<AvailabilityResponse> {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    let candidates_iter =
        std::iter::once(args.title.as_str()).chain(args.alt_titles.iter().map(String::as_str));
    for title in candidates_iter {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cands = scraper::search(&state.proxy_http, trimmed, mode, None).await?;
        if !cands.is_empty() {
            return Ok(AvailabilityResponse { available: true });
        }
    }
    Ok(AvailabilityResponse { available: false })
}
