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

/// TTL for title-match cache (Kitsu/AniList → allanime id).
pub const TITLE_MATCH_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttls_are_strictly_ordered_short_to_long() {
        // The hierarchy is fundamental to the cache discipline: trending is
        // the freshest signal, title-match is the most stable. Asserting
        // the order makes accidental swaps loud.
        assert!(TRENDING_TTL < DISCOVERY_TTL);
        assert!(DISCOVERY_TTL < ANIME_DETAIL_TTL);
        assert!(ANIME_DETAIL_TTL < TITLE_MATCH_TTL);
    }
}
