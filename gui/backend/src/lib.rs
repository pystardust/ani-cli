//! ani-gui — desktop GUI for the ani-cli anime scraper.
//!
//! This crate is the headless backend of the Electron application. It does
//! three things on the user's machine:
//!
//! 1. Drives the vendored `ani-cli` script (subprocess) to scrape allanime
//!    for search results, episode lists, and resolved stream URLs.
//! 2. Runs a localhost HTTP server that mounts (a) a streaming proxy which
//!    injects `Referer:` and rewrites m3u8 manifests so the embedded
//!    `<video>` + `hls.js` player can fetch segments without CORS pain,
//!    and (b) the API the renderer talks to via plain `fetch()`.
//! 3. Talks to Kitsu (and eventually AniList) for metadata, caches results
//!    in SQLite + images on disk, and reads the shared ani-cli history file.
//!
//! Every listening socket is bound to `127.0.0.1`. The Electron renderer
//! discovers the kernel-assigned port from the `ani-gui-backend` binary's
//! stdout handshake and talks to the API + proxy from there.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod anicli;
pub mod api;
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
