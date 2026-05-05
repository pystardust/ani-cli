//! ani-gui — desktop GUI for the ani-cli anime scraper.
//!
//! This crate is the Rust backend of the Tauri 2 application. It does three
//! things on the user's machine:
//!
//! 1. Drives the vendored `ani-cli` script (subprocess) to scrape allanime
//!    for search results, episode lists, and resolved stream URLs.
//! 2. Runs a localhost streaming proxy that injects the `Referer:` header
//!    every CDN requires and rewrites m3u8 manifests so the embedded
//!    `<video>` + `hls.js` player can fetch segments without CORS pain.
//! 3. Talks to Kitsu and AniList for metadata, caches results in SQLite +
//!    images on disk, and reads the shared ani-cli history file.
//!
//! No internet-reachable services are ever started — every listening socket
//! is bound to `127.0.0.1`. The crate's organizing principle is that the
//! frontend (SvelteKit, runs inside the Tauri webview) only ever talks to
//! Tauri commands and to the localhost proxy.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod anicli;
pub mod app;
pub mod cache;
pub mod commands;
pub mod config;
pub mod error;
pub mod history;
pub mod i18n;
pub mod meta;
pub mod proxy;

pub use error::{AniError, Result};

/// Library version, sourced from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_looks_like_semver() {
        // env!() produces a `&'static str`; clippy is right that
        // is_empty() on a const is silly. The check that matters is shape.
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(
            parts.len() >= 2,
            "CARGO_PKG_VERSION should be semver-shaped: {VERSION}"
        );
    }
}
