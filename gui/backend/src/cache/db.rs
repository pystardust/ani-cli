//! Connection pool + typed repos for the SQLite metadata cache.
//!
//! The pool wraps `r2d2_sqlite::SqliteConnectionManager`. On open, all
//! pending refinery migrations are applied (idempotent). Repos are free
//! functions that take `&SqlitePool` so they don't have to thread a
//! connection through the call site.
//!
//! ## TTL policy
//!
//! `meta_cache_get` returns `None` for entries past their TTL but does
//! not delete them — overwrite happens on the next `meta_cache_put`. This
//! lets a future revalidation flow opt to serve the stale body while a
//! background refresh runs.
//!
//! ## Threading
//!
//! Calls are synchronous. From async contexts, wrap in
//! `tokio::task::spawn_blocking` to avoid stalling the runtime.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension};

use crate::cache::schema::run_migrations;
use crate::error::{AniError, Result};

/// Convenience type alias for the pool.
pub type SqlitePool = Pool<SqliteConnectionManager>;

/// Open a pool against an on-disk SQLite database, creating the file if
/// it doesn't exist. Runs all pending migrations.
///
/// # Errors
/// - [`AniError::Cache`] when the pool can't be built or migrations fail.
pub fn open_pool(path: &Path) -> Result<SqlitePool> {
    let manager = SqliteConnectionManager::file(path);
    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|_| AniError::Cache)?;
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    run_migrations(&mut conn)?;
    Ok(pool)
}

/// Open a pool against an in-memory database. Tests only — `:memory:` is
/// per-connection in SQLite, so the pool is forced to `max_size(1)` so
/// the migration writes are visible to subsequent reads.
///
/// # Errors
/// - [`AniError::Cache`] when the pool can't be built or migrations fail.
pub fn open_in_memory() -> Result<SqlitePool> {
    let manager = SqliteConnectionManager::memory();
    let pool = Pool::builder()
        .max_size(1)
        .build(manager)
        .map_err(|_| AniError::Cache)?;
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    run_migrations(&mut conn)?;
    Ok(pool)
}

// --- meta_cache ----------------------------------------------------------

/// Fetch a meta_cache body if present and not expired.
///
/// # Errors
/// [`AniError::Cache`] on connection or query failure.
pub fn meta_cache_get(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let row: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT body, fetched_at, ttl_seconds FROM meta_cache WHERE key = ?1",
            params![key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|_| AniError::Cache)?;
    Ok(row.and_then(|(body, fetched_at, ttl)| {
        // `>=` so `ttl_seconds == 0` means "immediately expired"; the
        // common interpretation of a zero TTL across HTTP and most
        // cache libs.
        if now_secs().saturating_sub(fetched_at) >= ttl {
            None
        } else {
            Some(body)
        }
    }))
}

/// Insert or replace a meta_cache entry. `ttl_seconds` controls the
/// freshness window enforced by [`meta_cache_get`].
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_put(pool: &SqlitePool, key: &str, body: &str, ttl_seconds: u64) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let ttl_i64 = i64::try_from(ttl_seconds).unwrap_or(i64::MAX);
    conn.execute(
        "INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
         VALUES (?1, ?2, ?3, ?4)",
        params![key, body, now_secs(), ttl_i64],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Delete every meta_cache entry. Used by tests and a future "clear cache"
/// menu item.
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_clear(pool: &SqlitePool) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute("DELETE FROM meta_cache", [])
        .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Delete a single meta_cache entry. Used by feedback eviction (a
/// cached play resolution that the player just failed to load — drop
/// it so the next attempt re-fetches from upstream).
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_delete(pool: &SqlitePool, key: &str) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute("DELETE FROM meta_cache WHERE key = ?1", params![key])
        .map_err(|_| AniError::Cache)?;
    Ok(())
}

// --- title_match ---------------------------------------------------------

/// One row of the title_match table: a normalized user query string
/// resolved to one or both metadata-source ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleMatch {
    /// Lowercased + whitespace-collapsed user query.
    pub query_norm: String,
    /// Kitsu anime id (string in JSON:API).
    pub kitsu_id: Option<String>,
    /// AniList anime id (integer in GraphQL).
    pub anilist_id: Option<i64>,
    /// Unix epoch seconds the entry was last refreshed.
    pub fetched_at: i64,
}

/// Fetch a title_match row by normalized query.
///
/// # Errors
/// [`AniError::Cache`] on query failure.
pub fn title_match_get(pool: &SqlitePool, query_norm: &str) -> Result<Option<TitleMatch>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.query_row(
        "SELECT query_norm, kitsu_id, anilist_id, fetched_at FROM title_match WHERE query_norm = ?1",
        params![query_norm],
        |r| {
            Ok(TitleMatch {
                query_norm: r.get(0)?,
                kitsu_id: r.get(1)?,
                anilist_id: r.get(2)?,
                fetched_at: r.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|_| AniError::Cache)
}

/// Insert or replace a title_match row.
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn title_match_put(
    pool: &SqlitePool,
    query_norm: &str,
    kitsu_id: Option<&str>,
    anilist_id: Option<i64>,
) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "INSERT OR REPLACE INTO title_match(query_norm, kitsu_id, anilist_id, fetched_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![query_norm, kitsu_id, anilist_id, now_secs()],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

// --- helpers -------------------------------------------------------------

fn now_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_runs_migrations_and_creates_tables() {
        let pool = open_in_memory().expect("pool opens");
        let conn = pool.get().expect("checkout");
        // Migrations should have created all three tables.
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'refinery%' \
                 ORDER BY name",
            )
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(std::result::Result::unwrap)
            .collect();
        assert_eq!(tables, vec!["image_index", "meta_cache", "title_match"]);
    }

    #[test]
    fn meta_cache_round_trips_within_ttl() {
        let pool = open_in_memory().unwrap();
        meta_cache_put(&pool, "k", "{\"v\":1}", 60).unwrap();
        assert_eq!(
            meta_cache_get(&pool, "k").unwrap().as_deref(),
            Some("{\"v\":1}")
        );
    }

    #[test]
    fn meta_cache_returns_none_for_missing_keys() {
        let pool = open_in_memory().unwrap();
        assert_eq!(meta_cache_get(&pool, "missing").unwrap(), None);
    }

    #[test]
    fn meta_cache_returns_none_when_entry_is_past_ttl() {
        // ttl=0 means "expired immediately" per the `>=` semantics in
        // meta_cache_get; no sleep required and no raw-conn manipulation.
        let pool = open_in_memory().unwrap();
        meta_cache_put(&pool, "stale", "v", 0).unwrap();
        assert_eq!(meta_cache_get(&pool, "stale").unwrap(), None);
    }

    #[test]
    fn meta_cache_put_replaces_existing_value() {
        let pool = open_in_memory().unwrap();
        meta_cache_put(&pool, "k", "first", 60).unwrap();
        meta_cache_put(&pool, "k", "second", 60).unwrap();
        assert_eq!(
            meta_cache_get(&pool, "k").unwrap().as_deref(),
            Some("second")
        );
    }

    #[test]
    fn meta_cache_clear_removes_all_rows() {
        let pool = open_in_memory().unwrap();
        meta_cache_put(&pool, "a", "1", 60).unwrap();
        meta_cache_put(&pool, "b", "2", 60).unwrap();
        meta_cache_clear(&pool).unwrap();
        assert_eq!(meta_cache_get(&pool, "a").unwrap(), None);
        assert_eq!(meta_cache_get(&pool, "b").unwrap(), None);
    }

    #[test]
    fn title_match_round_trips_with_both_ids() {
        let pool = open_in_memory().unwrap();
        title_match_put(&pool, "one piece", Some("12"), Some(21)).unwrap();
        let row = title_match_get(&pool, "one piece")
            .unwrap()
            .expect("present");
        assert_eq!(row.kitsu_id.as_deref(), Some("12"));
        assert_eq!(row.anilist_id, Some(21));
        assert!(row.fetched_at > 0);
    }

    #[test]
    fn title_match_round_trips_with_only_one_id() {
        let pool = open_in_memory().unwrap();
        title_match_put(&pool, "obscure", Some("999"), None).unwrap();
        let row = title_match_get(&pool, "obscure").unwrap().expect("present");
        assert_eq!(row.kitsu_id.as_deref(), Some("999"));
        assert_eq!(row.anilist_id, None);
    }

    #[test]
    fn title_match_get_returns_none_for_missing_query() {
        let pool = open_in_memory().unwrap();
        assert!(title_match_get(&pool, "never seen").unwrap().is_none());
    }

    #[test]
    fn title_match_put_is_upsert() {
        let pool = open_in_memory().unwrap();
        title_match_put(&pool, "k", Some("first"), None).unwrap();
        title_match_put(&pool, "k", Some("second"), Some(42)).unwrap();
        let row = title_match_get(&pool, "k").unwrap().unwrap();
        assert_eq!(row.kitsu_id.as_deref(), Some("second"));
        assert_eq!(row.anilist_id, Some(42));
    }

    #[test]
    fn migrations_are_idempotent_across_pool_opens() {
        // Opening a second pool against the same in-memory DB would be
        // a fresh DB (max_size=1 + :memory: per-pool), so this test
        // instead verifies that calling the migration runner twice on
        // the same connection is a no-op.
        let pool = open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).expect("second run is a no-op");
        run_migrations(&mut conn).expect("third run is a no-op");
    }
}
