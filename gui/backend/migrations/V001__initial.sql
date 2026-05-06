-- Initial schema for the SQLite metadata + image-index cache.
--
-- Three tables, all keyed for primary-key lookups. Image *bytes* never
-- live in SQLite — only their index does; bytes go on disk under
-- `$XDG_CACHE_HOME/ani-gui/images/<hash[0..2]>/<hash>.<ext>`.

-- Generic key/value cache for serialized metadata bodies (Kitsu JSON,
-- AniList GraphQL responses, etc). TTL is enforced at read time:
-- entries are kept past expiry until a write replaces them, so a cache
-- hit on a stale entry can opt-in to revalidation.
CREATE TABLE meta_cache (
    key          TEXT PRIMARY KEY,
    body         TEXT NOT NULL,
    fetched_at   INTEGER NOT NULL,    -- unix epoch seconds
    ttl_seconds  INTEGER NOT NULL
);

CREATE INDEX meta_cache_fetched_at_idx ON meta_cache(fetched_at);

-- Stable mapping from "user query string" to the resolved Kitsu /
-- AniList ids that mapped to it. Lets the GUI skip a search round-trip
-- when the user picks the same title twice.
CREATE TABLE title_match (
    query_norm   TEXT PRIMARY KEY,
    kitsu_id     TEXT,
    anilist_id   INTEGER,
    fetched_at   INTEGER NOT NULL
);

-- Index of locally-cached image bytes. The hash is the lookup key the
-- frontend uses against the `image://` Tauri custom protocol; the
-- source_url + mime + bytes columns let us re-derive the on-disk path
-- and serve the right Content-Type header.
CREATE TABLE image_index (
    hash         TEXT PRIMARY KEY,
    source_url   TEXT NOT NULL,
    mime         TEXT,
    bytes        INTEGER,
    fetched_at   INTEGER NOT NULL
);

CREATE INDEX image_index_source_url_idx ON image_index(source_url);
