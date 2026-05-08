//! Eager image-cache warming for Kitsu API responses.
//!
//! Some Kitsu entries (recently-aired sequels with no curated cover
//! art) ship `posterImage.original` as a Backblaze S3 *presigned URL*
//! with a 15-minute signature window. Our Kitsu response cache lives
//! for hours-to-days, so by the time the renderer renders the URL the
//! signature is stale and S3 returns 403 → broken image.
//!
//! The fix runs at cache write time: as soon as we successfully cache
//! a Kitsu response, we walk the body for any URL containing
//! `X-Amz-Signature=`, fire `meta::images::get_or_fetch` for each one,
//! and let the image cache store the bytes under its canonical
//! (signature-stripped) key. The renderer's later `<img src=…>`
//! request hits the byte cache regardless of whether the URL it sees
//! is fresh.

use crate::app::AppState;

/// Extract every URL string in `body` that contains the
/// `X-Amz-Signature=` query param. URLs are assumed to be inside
/// JSON-style double quotes — that's how Kitsu responses ship them.
/// Returns deduplicated URLs in order of first appearance.
#[must_use]
pub fn extract_x_amz_urls(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let needle = "X-Amz-Signature=";
    let mut idx = 0;
    while let Some(off) = body[idx..].find(needle) {
        let sig_pos = idx + off;
        // Find the surrounding JSON-string quotes. URLs in Kitsu
        // bodies are always quoted; everything else (a stray
        // `X-Amz-Signature=` in a comment, say) we'd rather skip.
        let head = body[..sig_pos].rfind('"');
        let tail = body[sig_pos..].find('"').map(|t| sig_pos + t);
        if let (Some(h), Some(t)) = (head, tail) {
            // Discard the quotes themselves; require the URL to look
            // http-ish so we don't pick up arbitrary signed strings.
            let candidate = &body[h + 1..t];
            if (candidate.starts_with("https://") || candidate.starts_with("http://"))
                && !out.iter().any(|u| u == candidate)
            {
                out.push(candidate.to_string());
            }
            idx = t + 1;
        } else {
            break;
        }
    }
    out
}

/// Spawn a fire-and-forget `get_or_fetch` task for every signed URL
/// in `body`. Errors are swallowed (logged): the warm is opportunistic
/// — failure means the renderer falls back to whatever it would have
/// shown without the warm, no worse than today's behavior.
pub fn warm_signed_image_urls(state: &AppState, body: &str) {
    let urls = extract_x_amz_urls(body);
    if urls.is_empty() {
        return;
    }
    let cap_bytes = state.image_cache_cap_bytes();
    for url in urls {
        let client = state.proxy_http.clone();
        let cache_dir = state.image_cache_dir.clone();
        tokio::spawn(async move {
            match crate::meta::images::get_or_fetch(&client, &cache_dir, &url).await {
                Ok(_) => {
                    crate::meta::images::schedule_prune(cache_dir, cap_bytes);
                }
                Err(e) => {
                    tracing::warn!(
                        url = %url,
                        error = ?e,
                        "image-cache warm failed",
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_finds_a_single_signed_url_inside_json_quotes() {
        let body = r#"{"posterImage":{"original":"https://kitsu-prod.s3.us-west-002.backblazeb2.com/anime/48069/poster.jpg?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Date=20260508T013024Z&X-Amz-Expires=900&X-Amz-Signature=abc123"}}"#;
        let urls = extract_x_amz_urls(body);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].starts_with("https://kitsu-prod.s3.us-west-002.backblazeb2.com/"));
        assert!(urls[0].contains("X-Amz-Signature=abc123"));
    }

    #[test]
    fn extract_returns_empty_when_no_signature_present() {
        // Standard CDN URLs (most Kitsu entries) — no warming needed.
        let body = r#"{"posterImage":{"large":"https://media.kitsu.app/anime/12/poster.jpg"}}"#;
        assert!(extract_x_amz_urls(body).is_empty());
    }

    #[test]
    fn extract_finds_multiple_signed_urls_and_dedupes() {
        // A response containing both a poster and a cover, both
        // signed; same URL appearing twice (e.g. duplicate fields)
        // should appear once in the output.
        let body = r#"{
            "posterImage":{"original":"https://s3.example/p.jpg?X-Amz-Signature=A"},
            "coverImage":{"original":"https://s3.example/c.jpg?X-Amz-Signature=B"},
            "duplicate":"https://s3.example/p.jpg?X-Amz-Signature=A"
        }"#;
        let urls = extract_x_amz_urls(body);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("p.jpg"));
        assert!(urls[1].contains("c.jpg"));
    }

    #[test]
    fn extract_skips_unquoted_signature_text() {
        // Defensive — a stray `X-Amz-Signature=…` in free text (a
        // comment field, say) without surrounding quotes shouldn't
        // be picked up as a URL.
        let body = "free text X-Amz-Signature=foo end";
        assert!(extract_x_amz_urls(body).is_empty());
    }

    #[test]
    fn extract_only_returns_http_or_https() {
        // Quoted file:// or other schemes containing X-Amz-Signature
        // (vanishingly unlikely but defensive) shouldn't be fetched.
        let body = r#""file:///tmp/x?X-Amz-Signature=oops""#;
        assert!(extract_x_amz_urls(body).is_empty());
    }
}
