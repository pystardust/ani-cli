//! Outbound `reqwest` client for the streaming proxy.
//!
//! Separate from the `meta_http` client (Kitsu/AniList) so connection
//! pooling and retry policy can differ — stream segments are large and
//! latency-sensitive, metadata calls are small and cacheable.

// Implementation lands in M1.3.
