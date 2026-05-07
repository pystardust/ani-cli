//! Long-term cache for play resolutions.
//!
//! Caches the *result* of running ani-cli (upstream URL, referer,
//! subtitle URL, media kind) keyed by `(canonical_title, mode, quality,
//! episode)`. A subsequent click on the same episode skips the 30s
//! ani-cli spawn entirely — we just register a fresh proxy session
//! around the cached upstream and return immediately.
//!
//! Two safeties:
//! 1. **TTL**: 24h ([`PLAY_RESOLUTION_TTL`]) — wixmp / sharepoint URLs
//!    rotate, so caching longer means more dead links on hit.
//! 2. **HEAD validation on read** — a quick HEAD with the captured
//!    Referer confirms the upstream is still alive before we serve it.
//!    On any failure (network / 4xx / 5xx) we evict the entry and let
//!    the caller fall through to ani-cli.
//!
//! ### Why we don't cache pre-session
//!
//! `session_id` is generated per-call and lives in the proxy's
//! [`SessionTable`]. Caching the session itself would require
//! invalidating sessions on cache eviction and ensuring TTLs match.
//! Easier to cache the raw resolution (the data ani-cli produced) and
//! rebuild the session on every play. The session's own TTL (4h)
//! handles GC of the proxy table.
//!
//! ### Why this doesn't cover the first-visit slow click
//!
//! The cache only helps subsequent plays of the same episode — first
//! play is still a fresh ani-cli spawn (~30s). The prefetch in
//! `play-cache.ts` warms the cache for nearby episodes in the
//! background; this cache then makes the *next* visit to the same
//! show instant.

use serde::{Deserialize, Serialize};

use crate::cache::ttl::PLAY_RESOLUTION_TTL;
use crate::cache::{meta_cache_delete, meta_cache_get, meta_cache_put, SqlitePool};
use crate::proxy::MediaKind;

/// Schema version for cached play resolutions. Bump when the struct
/// gains a field consumers depend on; old keys become misses on first
/// access and re-fetch with the new schema.
///
/// Bump history:
/// - v1: original (upstream_url, referer, subtitle_url, media_kind).
/// - v2: added `show_id` + `show_title` so cache hits can also update
///   `ani-hsts` / Continue Watching. v1 rows had no metadata to write
///   the history line with — bumping forces a re-resolve so the new
///   fields populate naturally.
const SCHEMA: &str = "v2";

/// What ani-cli's debug output produced, frozen for replay. The session
/// layer rebuilds a fresh `StreamSession` from this on cache hit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedResolution {
    /// Final upstream URL ani-cli selected (after `select_quality`).
    pub upstream_url: String,
    /// Referer header to send when fetching upstream. Empty string when
    /// no referer was captured / inferred.
    pub referer: String,
    /// Subtitle URL when ani-cli surfaced one, else `None`.
    pub subtitle_url: Option<String>,
    /// Whether the proxy should serve this as HLS (manifest rewrite)
    /// or MP4 (byte-stream pass-through).
    pub media_kind: MediaKind,
    /// Allanime show id of the chosen candidate. Captured during the
    /// fresh-fetch path so a subsequent cache-hit can update
    /// `ani-hsts` (which `ani-cli`'s `update_history` keys on this id).
    /// Empty string on rows written before this field existed —
    /// callers fall back to skipping the history write.
    #[serde(default)]
    pub show_id: String,
    /// Allanime title of the chosen candidate, including the
    /// `(N episodes)` parenthetical that `ani-cli`'s `update_history`
    /// stores in column three. Empty on legacy rows.
    #[serde(default)]
    pub show_title: String,
}

/// Build the SQLite key for a play resolution. Deterministic per
/// `(title, mode, quality, episode)` — alt_titles are intentionally
/// excluded because the cache is keyed on what the *frontend* asks
/// for, not the title that ultimately matched allmanga. Same query →
/// same key on the next visit, even after we've fixed Kitsu cache
/// schemas etc.
#[must_use]
pub fn cache_key(title: &str, mode: &str, quality: &str, episode: &str) -> String {
    // `:` is the table convention. The fields don't contain it
    // (mode/quality are enums, episode is digits), so no escaping
    // needed for them. Title can contain `:` (Stone Ocean Part 2
    // canonical has one). It's still unambiguous given the field
    // count, and serde_json never tries to parse this — it's only a
    // SQLite text key.
    format!("play:{SCHEMA}:{title}:{mode}:{quality}:{episode}")
}

/// Look up a cached resolution. Returns `Ok(None)` on miss or expired.
/// The HEAD-validation step is the caller's responsibility — this
/// helper only handles the SQLite read + JSON parse.
///
/// # Errors
/// SQLite errors propagate; deserialization failures are treated as
/// misses (we don't want a corrupt row to permanently mask a show).
pub fn get(pool: &SqlitePool, key: &str) -> crate::error::Result<Option<CachedResolution>> {
    match meta_cache_get(pool, key)? {
        None => Ok(None),
        Some(body) => Ok(serde_json::from_str(&body).ok()),
    }
}

/// Persist a fresh resolution under [`PLAY_RESOLUTION_TTL`]. Errors
/// from SQLite or serialization are logged at warn level by the
/// caller; we don't propagate (a cache write failure shouldn't fail
/// the play).
pub fn put(pool: &SqlitePool, key: &str, value: &CachedResolution) {
    let Ok(body) = serde_json::to_string(value) else {
        return;
    };
    let _ = meta_cache_put(pool, key, &body, PLAY_RESOLUTION_TTL.as_secs());
}

/// Drop a single cached resolution. Two callers feed this:
/// 1. The play flow's HEAD-fail branch — the cached URL is dead so
///    the row should not linger if the fresh ani-cli call also fails.
/// 2. The frontend's player-error feedback path — the player tried
///    the cached upstream and got a 4xx/5xx, so the URL has rotated
///    even though our HEAD said it was alive.
///
/// Errors are swallowed (best-effort) for the same reason `put` does:
/// a delete failure shouldn't propagate to the user.
pub fn evict(pool: &SqlitePool, key: &str) {
    let _ = meta_cache_delete(pool, key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::open_in_memory;

    fn pool() -> SqlitePool {
        open_in_memory().expect("in-memory pool")
    }

    fn sample_resolution() -> CachedResolution {
        CachedResolution {
            upstream_url:
                "https://video.wixstatic.com/video/3d2d69_c12bd6c53e234420b3ae3d3b4c5b526f/1080p/mp4/file.mp4"
                    .into(),
            referer: "https://allmanga.to".into(),
            subtitle_url: None,
            media_kind: MediaKind::Mp4,
            show_id: "vDTSJHSpYnrkZnAvG".into(),
            show_title: "Naruto: Shippuuden (500 episodes)".into(),
        }
    }

    #[test]
    fn cache_key_is_deterministic_for_the_same_inputs() {
        let a = cache_key("One Piece", "sub", "best", "1");
        let b = cache_key("One Piece", "sub", "best", "1");
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_across_each_axis() {
        let base = cache_key("One Piece", "sub", "best", "1");
        assert_ne!(cache_key("Naruto", "sub", "best", "1"), base);
        assert_ne!(cache_key("One Piece", "dub", "best", "1"), base);
        assert_ne!(cache_key("One Piece", "sub", "1080", "1"), base);
        assert_ne!(cache_key("One Piece", "sub", "best", "2"), base);
    }

    #[test]
    fn cache_key_includes_schema_version_so_v0_entries_are_unreachable() {
        // Bump SCHEMA when CachedResolution gains a field consumers
        // depend on; old un-versioned-or-older-versioned entries
        // become misses on first access. This test pins the prefix
        // shape so a typo in SCHEMA doesn't silently produce keys
        // that collide with the prior version.
        let k = cache_key("X", "sub", "best", "1");
        assert!(k.starts_with("play:v2:"), "got {k}");
    }

    #[test]
    fn put_then_get_round_trips_the_resolution() {
        let pool = pool();
        let key = cache_key("Stone Ocean", "sub", "best", "1");
        put(&pool, &key, &sample_resolution());
        let got = get(&pool, &key).expect("ok").expect("hit");
        assert_eq!(got, sample_resolution());
    }

    #[test]
    fn get_returns_none_on_miss() {
        let pool = pool();
        let got = get(&pool, "play:v2:Nope:sub:best:1").expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn evict_removes_a_row_so_subsequent_get_misses() {
        let pool = pool();
        let key = cache_key("Stone Ocean", "sub", "best", "1");
        put(&pool, &key, &sample_resolution());
        assert!(get(&pool, &key).expect("ok").is_some());
        evict(&pool, &key);
        assert!(
            get(&pool, &key).expect("ok").is_none(),
            "evict() must wipe the row, not just expire it"
        );
    }

    #[test]
    fn evict_is_idempotent_on_missing_key() {
        let pool = pool();
        // Eviction by frontend feedback may race the natural
        // eviction-on-HEAD-fail in the backend. Both callers should
        // be safe to invoke even when the row is already gone.
        evict(&pool, "play:v2:Never:Cached:best:1");
        assert!(get(&pool, "play:v2:Never:Cached:best:1")
            .expect("ok")
            .is_none());
    }

    #[test]
    fn get_parses_legacy_rows_missing_show_id_and_title() {
        // Rows written before the show_id/show_title fields existed
        // must still deserialize — serde_default fills in empty strings
        // and the cache-hit path skips the history-write when those
        // are blank. Without this, the bump to v2 of CachedResolution
        // would silently invalidate every row.
        let pool = pool();
        let key = "play:v2:Legacy:sub:best:1";
        let legacy = r#"{"upstream_url":"https://x/y.mp4","referer":"","subtitle_url":null,"media_kind":"mp4"}"#;
        meta_cache_put(&pool, key, legacy, 60).unwrap();
        let got = get(&pool, key).expect("ok").expect("hit");
        assert_eq!(got.show_id, "");
        assert_eq!(got.show_title, "");
        assert_eq!(got.upstream_url, "https://x/y.mp4");
    }

    #[test]
    fn get_treats_corrupt_payload_as_miss() {
        // A migrated payload from a future version, or an externally
        // edited row, shouldn't permanently mask the show — the play
        // flow should fall through to ani-cli and overwrite the row.
        let pool = pool();
        let key = "play:v2:Garbage:sub:best:1";
        meta_cache_put(&pool, key, "{ not valid json", 60).unwrap();
        assert!(get(&pool, key).expect("ok").is_none());
    }
}
