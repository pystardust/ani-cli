//! Local stream proxy for the embedded `<video>` + hls.js player.
//!
//! Bound to `127.0.0.1:0` (kernel-assigned port) at app startup. Routes:
//!
//! - `GET /healthz` — liveness probe used by the frontend bootstrap.
//! - `GET /s/<token>/master.m3u8` — fetches the upstream master playlist,
//!   rewrites every variant + segment URI to flow through the proxy with
//!   HMAC-signed sub-tokens, and returns the rewritten manifest.
//! - `GET /s/<token>/seg?u=<base64-url>` — proxies an individual segment
//!   with the right `Referer:` header.
//! - `GET /s/<token>/sub.vtt` — proxies the subtitle file.
//!
//! All filled in across M1.3 with TDD coverage.

pub mod m3u8;
pub mod token;
pub mod upstream;
