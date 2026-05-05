//! HMAC-signed stream session tokens.
//!
//! When the backend resolves a stream URL via [`crate::anicli::process::run_debug`],
//! it creates a `StreamSession` and returns a UUID. Segment URLs the
//! frontend's hls.js fetches are signed so the proxy can verify the
//! request without re-checking the session table per segment.
//!
//! Property tests target the encode/decode roundtrip and TTL handling.

// Implementation lands in M1.3.
