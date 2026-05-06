//! Metadata client for Kitsu and AniList.
//!
//! Kitsu is the primary source for everything except the Trending Now row;
//! AniList is used only for trending. See `docs/architecture.md` §discovery.
//!
//! Implementation lands in M2 (Kitsu) and M3 (AniList).

pub mod anilist;
pub mod images;
pub mod kitsu;
