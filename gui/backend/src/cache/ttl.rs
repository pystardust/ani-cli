//! TTL policy per cache key prefix.
//!
//! See `docs/architecture.md` §"Caching" for the table.

use std::time::Duration;

/// TTL for the AniList trending row.
pub const TRENDING_TTL: Duration = Duration::from_secs(60 * 60); // 1h

/// TTL for Kitsu seasonal / top / recent rows.
pub const DISCOVERY_TTL: Duration = Duration::from_secs(6 * 60 * 60); // 6h

/// TTL for per-anime metadata (`/anime/:id`).
pub const ANIME_DETAIL_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7d

/// TTL for episode lists. Shorter than ANIME_DETAIL_TTL because new
/// episodes appear weekly for currently-airing shows; a 1-day window
/// caps staleness at a manageable level without burning Kitsu calls.
/// For finished shows, the data is stable, but re-fetching once a day
/// is cheap.
pub const EPISODES_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 1d

/// TTL for title-match cache (Kitsu/AniList → allanime id).
pub const TITLE_MATCH_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

/// TTL for cached play resolutions (canonical_title + mode + quality +
/// episode → upstream URL + referer + subtitle + media_kind). 7d is
/// generous because the *real* validity gate is the per-read HEAD
/// check + an evict-on-player-failure feedback loop — a stale row
/// survives until the next access, where HEAD or playback failure
/// kicks it out. The TTL is just a backstop for rows nobody touches
/// for a week (probably abandoned shows).
pub const PLAY_RESOLUTION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7d

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttls_are_strictly_ordered_short_to_long() {
        // The hierarchy is fundamental to the cache discipline: trending is
        // the freshest signal, title-match is the most stable. Asserting
        // the order makes accidental swaps loud.
        assert!(TRENDING_TTL < DISCOVERY_TTL);
        assert!(DISCOVERY_TTL < EPISODES_TTL);
        assert!(EPISODES_TTL < ANIME_DETAIL_TTL);
        assert!(ANIME_DETAIL_TTL < TITLE_MATCH_TTL);
    }
}
