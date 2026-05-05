//! HLS manifest rewriting.
//!
//! Pure functions over `m3u8-rs`'s parsed types. Property test target
//! (idempotency: `rewrite(rewrite(x)) == rewrite(x)`).

// Implementation lands in M1.3.
