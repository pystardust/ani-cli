//! `infer_referer` — picks the `Referer:` value to forward to the
//! external player (or Syncplay's wrapped player) based on
//! `ani-cli`'s parsed debug output.
//!
//! Extracted from `commands/play.rs` for two reasons:
//!   1. play.rs was carrying ~10 ccn of related test scaffolding
//!      (5 inline tests + a `debug()` fixture builder), pushing
//!      the file's CRAP score past the firm 356 ceiling.
//!   2. Per-host referer overrides are a stable concern with one
//!      explicit input shape — moving them out of the play flow
//!      keeps the play module focused on resolution + spawn.

use crate::anicli::parser::DebugOutput;

/// Pick the `Referer:` value to forward to the player. Trust
/// `resolved.referer` when ani-cli surfaced one; otherwise fall back
/// to a per-host default — fast4speed.rsvp 403s without
/// `Referer: https://allmanga.to` and ani-cli's debug output doesn't
/// expose the header it sets internally. The play and play_external
/// resolve-paths call this; play_syncplay reuses the cache row's
/// referer on cache-hit and only hits this helper on cache-miss.
#[must_use]
pub(super) fn infer_referer(resolved: &DebugOutput) -> Option<String> {
    if let Some(r) = resolved.referer.as_ref() {
        if !r.is_empty() {
            return Some(r.clone());
        }
    }
    let host = url::Url::parse(&resolved.selected_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))?;
    if host.ends_with("fast4speed.rsvp") {
        Some("https://allmanga.to".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debug(selected_url: &str, referer: Option<&str>) -> DebugOutput {
        DebugOutput {
            selected_url: selected_url.into(),
            all_links: vec![],
            referer: referer.map(str::to_string),
            subtitle_url: None,
        }
    }

    #[test]
    fn infer_referer_trusts_explicit_value_from_ani_cli() {
        // When ani-cli surfaces a referer in its debug output, that's
        // the catalogue-correct one — use it verbatim regardless of
        // the upstream host. Empty-string referer falls through to
        // the host-based inference (treated as missing).
        let got = infer_referer(&debug(
            "https://example.com/v.mp4",
            Some("https://example.com"),
        ));
        assert_eq!(got, Some("https://example.com".to_string()));
    }

    #[test]
    fn infer_referer_falls_back_to_allmanga_for_fast4speed() {
        // fast4speed.rsvp 403s without Referer: https://allmanga.to —
        // ani-cli's debug output doesn't surface the header it sets
        // internally, so we re-derive it from the host. Without this
        // fallback, mpv (and Syncplay's wrapped mpv) can't play
        // fast4speed streams.
        let got = infer_referer(&debug("https://tools.fast4speed.rsvp/v.mp4", None));
        assert_eq!(got, Some("https://allmanga.to".to_string()));
    }

    #[test]
    fn infer_referer_handles_empty_string_as_missing() {
        // An empty-string referer from ani-cli is no better than
        // None — fall through to the host-based inference.
        let got = infer_referer(&debug("https://tools.fast4speed.rsvp/v.mp4", Some("")));
        assert_eq!(got, Some("https://allmanga.to".to_string()));
    }

    #[test]
    fn infer_referer_returns_none_for_unknown_host_without_referer() {
        // Hosts not on the fast4speed.rsvp allowlist get None — most
        // CDNs don't require a Referer at all, and guessing one would
        // be worse than leaving the field unset.
        let got = infer_referer(&debug("https://video.wixstatic.com/v.mp4", None));
        assert_eq!(got, None);
    }

    #[test]
    fn infer_referer_returns_none_for_unparseable_url_without_referer() {
        // Defensive: an upstream URL that doesn't parse shouldn't
        // crash. Fall through to None — the player will fail to load
        // the URL anyway, but with a clean error instead of a panic.
        let got = infer_referer(&debug("not a url", None));
        assert_eq!(got, None);
    }
}
