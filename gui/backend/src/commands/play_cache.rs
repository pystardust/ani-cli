//! Cache-shaped helpers extracted from `commands::play` so that
//! file's cyclomatic complexity stays under the CRAP ratchet. Three
//! related helpers live here:
//!
//!   • `upstream_head_ok` — HEAD-pings a cached upstream URL with
//!     the right Referer and treats 2xx/3xx as live, anything else
//!     (including network errors) as dead.
//!   • `try_serve_cached` — turns a `CachedResolution` row into a
//!     fresh session response when its URL still passes
//!     `upstream_head_ok`. Used by the embedded-player flow's
//!     fast path in `play_with_progress`.
//!   • `try_launch_args_from_cache` — sibling of `try_serve_cached`
//!     for the external-player flow: walks the same cache, HEAD-
//!     validates, and returns ready-to-launch [`LaunchArgs`] (or
//!     `None`) so `play_external` can hand mpv a cached URL without
//!     re-spawning ani-cli.
//!
//! All three are async and depend on `AppState`'s reqwest client +
//! cache pool, so the fixtures in play.rs's test module
//! (`state_with_proxy_origin`, `cached_blank`, `seed_play_cache`)
//! are still used to drive them via wiremock.
//!
//! No behaviour change relative to the previous in-`play.rs`
//! definitions — pure relocation to drop the file's reported CCN.

use crate::app::AppState;
use crate::commands::external_player::LaunchArgs;
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::session::{
    create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};

/// HEAD-validate that `url` is still alive, with the supplied
/// `referer` (empty string means "no Referer header"). 2xx and 3xx
/// (CDN edge redirects) both count as live; everything else,
/// including network errors, is dead.
pub(crate) async fn upstream_head_ok(
    client: &reqwest::Client,
    url: &url::Url,
    referer: &str,
) -> bool {
    let mut req = client.head(url.as_str());
    if !referer.is_empty() {
        req = req.header(reqwest::header::REFERER, referer);
    }
    let Ok(resp) = req.send().await else {
        return false;
    };
    resp.status().is_success() || resp.status().is_redirection()
}

/// HEAD-validate a cached upstream URL. Returns a fresh
/// CreateSessionResponse on success, or `None` if the URL is dead /
/// unreachable / returns an error status — caller should fall through
/// to a fresh ani-cli spawn.
pub(crate) async fn try_serve_cached(
    state: &AppState,
    cached: &CachedResolution,
) -> Option<CreateSessionResponse> {
    let url = url::Url::parse(&cached.upstream_url).ok()?;
    if !upstream_head_ok(&state.proxy_http, &url, &cached.referer).await {
        return None;
    }
    let session_args = CreateSessionArgs {
        upstream_url: cached.upstream_url.clone(),
        referer: cached.referer.clone(),
        subtitle_url: cached.subtitle_url.clone(),
    };
    let mut resp = create_session_with_kind(state, &session_args, cached.media_kind).ok()?;
    // Tag so the renderer can decide whether a player error is
    // retryable (cache hit can be evicted + re-resolved) or terminal
    // (fresh fetch, no cache to clear).
    resp.cache_hit = true;
    Some(resp)
}

/// Cache-hit branch of `play_external`: returns ready-to-launch
/// `LaunchArgs` when the play_resolution_cache has a live row,
/// otherwise `None` (caller falls through to a fresh ani-cli spawn).
/// HEAD-fail evicts the row before returning None so the next
/// attempt isn't bitten by the same dead URL.
pub(crate) async fn try_launch_args_from_cache(
    state: &AppState,
    args: &super::play::PlayArgs,
    cfg: &crate::config::Config,
) -> Option<LaunchArgs> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cache_key =
        play_resolution_cache::cache_key(&args.title, &args.mode, quality, &args.episode);
    let cached = play_resolution_cache::get(&state.cache_pool, &cache_key).ok()??;
    let parsed = url::Url::parse(&cached.upstream_url).ok()?;
    if !upstream_head_ok(&state.proxy_http, &parsed, &cached.referer).await {
        play_resolution_cache::evict(&state.cache_pool, &cache_key);
        tracing::info!(
            title = %args.title,
            episode = %args.episode,
            "play_external: cache row stale (HEAD failed), evicted, falling back to ani-cli",
        );
        return None;
    }
    tracing::info!(
        title = %args.title,
        episode = %args.episode,
        upstream = cached.upstream_url.as_str(),
        "play_external: cache hit (HEAD ok), launching mpv from cached URL",
    );
    Some(LaunchArgs {
        stream_url: cached.upstream_url,
        referer: if cached.referer.is_empty() {
            None
        } else {
            Some(cached.referer)
        },
        subtitle_url: cached.subtitle_url,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player.clone(),
        // TODO(green): plumb cfg.external_player_kind +
        // cfg.external_player_custom_args once those fields land.
        player_kind: crate::commands::external_player::ExternalPlayerKind::Mpv,
        custom_args_template: None,
    })
}
