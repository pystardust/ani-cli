//! SQLite metadata cache + on-disk image cache.
//!
//! Schema is embedded via `refinery` migrations; the connection pool uses
//! `r2d2_sqlite`. Cache reads are sync — wrap calls in
//! `tokio::task::spawn_blocking` from async contexts.
//!
//! Tables (defined in `schema.rs` once M2 lands):
//!
//! - `meta_cache(key TEXT PRIMARY KEY, body TEXT, fetched_at INTEGER, ttl_seconds INTEGER)`
//! - `title_match(query_norm TEXT PRIMARY KEY, kitsu_id TEXT, anilist_id INTEGER, fetched_at INTEGER)`
//! - `image_index(hash TEXT PRIMARY KEY, source_url TEXT, mime TEXT, bytes INTEGER, fetched_at INTEGER)`
//!
//! Image bytes never live in SQLite; only the index does. Files live at
//! `$XDG_CACHE_HOME/ani-gui/images/<hash[0..2]>/<hash>.<ext>`.

pub mod db;
pub mod schema;
pub mod ttl;

pub use db::{
    meta_cache_clear, meta_cache_get, meta_cache_put, open_in_memory, open_pool, title_match_get,
    title_match_put, SqlitePool, TitleMatch,
};
