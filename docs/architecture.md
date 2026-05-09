# Architecture

`ani-gui` is a desktop app that lets you browse and watch anime through a graphical interface. Under the hood it reuses the [`ani-cli`](https://github.com/pystardust/ani-cli) Bash scraper unmodified and adds a Rust + SvelteKit frontend on top of it.

## What gets shipped

A single-window desktop application. Linux: AppImage, `.deb`, Flatpak. macOS: `.dmg`. Windows: `.msi`. The user double-clicks an icon and gets a native window. There is no URL to visit, no port to remember, no internet-reachable service.

## Why there is a "backend"

The app talks to three things a browser tab cannot reach on its own:

1. The vendored `ani-cli` script — needs a subprocess.
2. The shared history file at `$XDG_STATE_HOME/ani-cli/ani-hsts` — needs filesystem access.
3. Anime stream CDNs — require a `Referer:` header that browser fetch APIs cannot set, and serve segments without permissive CORS.

So the app embeds a Rust backend, bound to `127.0.0.1` on a kernel-assigned port, that orchestrates these pieces. It runs as a sidecar process the desktop shell launches at startup — a localhost daemon, not a server anyone else can reach.

## Components

```
 ┌──────────────────────────────────────────────────────────────┐
 │  ani-gui desktop app                                         │
 │                                                              │
 │  ┌────────────────────┐   fetch()   ┌────────────────────┐   │
 │  │ Renderer (SPA)     │ ──────────► │ Backend (sidecar)  │   │
 │  │ SvelteKit + hls.js │ ◄────────── │ Rust HTTP server   │   │
 │  └─────────┬──────────┘             └─────┬──────────────┘   │
 │            │                              │                  │
 │            │ <video src="http://127.0.0.1:├──► ani-cli       │
 │            │  PORT/s/<token>/...">        │   subprocess     │
 │            │                              │                  │
 │            │  bytes streamed via proxy    ├──► Kitsu (REST)  │
 │            └─────────────────────────────►│   AniList (GQL)  │
 │                  Referer + CORS           │                  │
 │                                           ├──► history TSV   │
 │                                           │                  │
 │                                           └──► SQLite +      │
 │                                              image cache     │
 └──────────────────────────────────────────────────────────────┘

 sibling: pystardust/ani-cli (vendored, untouched)
```

Three layers, in lockstep:

- **Renderer** — SvelteKit static SPA running inside the desktop shell's web view. Renders the discovery surface, search results, detail pages, and the embedded player (`<video>` + hls.js). Stateless beyond UI state; talks only to the backend.
- **Backend** — Rust crate inside `gui/backend/`. Spawned as a sidecar by the desktop shell at startup. Spawns `ani-cli` as a subprocess, fetches metadata from Kitsu/AniList, reads/writes the shared history file, runs a streaming proxy on a localhost port, and exposes an HTTP API the renderer talks to via `fetch()`.
- **External processes** — `ani-cli` (the script) and optionally `mpv` for the "Open in external player" escape hatch.

## Data flow: searching and playing an episode

1. The user types a query into the search bar.
2. The renderer calls `POST /api/kitsu/search`. The backend hits Kitsu and returns matches.
3. The user picks a result; the renderer fetches detail and episode list via `GET /api/kitsu/anime/:id` and `GET /api/kitsu/episodes/:id`.
4. The user clicks an episode. The renderer calls `POST /api/sessions` with the chosen anime + episode. The backend spawns `ani-cli` with `ANI_CLI_PLAYER=debug` and `-e <ep>`, parses the resolved stream URL plus its `Referer` requirement and any subtitle `.vtt` URL.
5. The backend creates a `StreamSession` (UUID, upstream URL, referer, expiry), stores it in memory, and returns a token to the renderer.
6. The renderer mounts `<video>` and points hls.js at `http://127.0.0.1:<port>/s/<token>/master.m3u8`.
7. The streaming proxy fetches the upstream master playlist with the correct `Referer:` header, parses it with `m3u8-rs`, and rewrites every variant + segment URI to flow back through itself with HMAC-signed sub-tokens. CORS headers are added so hls.js inside the webview can consume the rewritten manifest without preflight blocks.
8. Subsequent segment requests follow the same path: hls.js asks the proxy, the proxy asks the upstream with the `Referer:`, bytes stream back.

## Discovery (landing page)

The landing page shows four rows: **Trending Now**, **Popular This Season**, **Top Rated**, **Recently Released**.

- **Trending Now** is fetched from AniList's GraphQL endpoint (`Page.media(sort: TRENDING_DESC)`). AniList's trending sort tracks current weekly popularity, which Kitsu's `userCount` cannot match.
- **Popular This Season**, **Top Rated**, and **Recently Released** are fetched from Kitsu (REST/JSON:API). Kitsu's posters and banners (sizes verified at build time) are sufficient for these views.
- Both APIs are hit only when cache misses; cache is SQLite (`$XDG_CACHE_HOME/ani-gui/meta.db`) with TTLs from 1 hour (trending) up to 30 days (title-match cache).

When a user clicks a discovery card, the backend resolves its title against `ani-cli` (search by every available alias from the metadata API: English, Romaji, Native, synonyms) and falls into the same playback flow. The cross-API bridge — including how Kitsu's episode count disambiguates colliding titles on allmanga, and how the MAL id is fetched for the aniskip and trending lookups — is documented in [`title-resolution.md`](./title-resolution.md).

When Kitsu's `coverImage` is null (common for shows currently airing — roughly half of the trending row in any given week), the detail-page resolver falls back to AniList: it bridges the Kitsu id through the mappings endpoint to a MAL id, then queries AniList for that MAL id's `bannerImage`. Without the fallback the detail page would render a flat colour where the hero banner belongs.

## Caching

| Asset | Storage | TTL |
|---|---|---|
| AniList trending row | SQLite `meta_cache` | 1 hour |
| Kitsu seasonal / top / recent | SQLite `meta_cache` | 6 hours |
| Per-anime metadata (`/anime/:id`) | SQLite `meta_cache` | 7 days |
| Availability probe (positive, ongoing show) | SQLite `meta_cache` | 24 hours |
| Availability probe (positive, finished show) | SQLite `meta_cache` | 30 days |
| Availability probe (negative — show isn't on allmanga) | SQLite `meta_cache` | 7 days |
| aniskip OP/ED skip-time intervals (per MAL id + episode) | SQLite `meta_cache` | 7 days |
| Title matches (Kitsu/AniList → allanime ID) | SQLite `title_match` | 30 days |
| Long-term play resolution (resolved stream URLs) | SQLite play-resolution table | until upstream rotates |
| Poster + banner image bytes | Filesystem `images/<shard>/<hash>.<ext>` | LRU, capped at 500 MB |

Image bytes never live in SQLite; they're filesystem-keyed by `sha256(url)[..16]`, sharded two-deep to avoid huge flat directories. The play-resolution cache is separate from `meta_cache` — it stores fully-resolved stream URLs keyed by `(allanime id, episode, mode, quality)` so a repeat visit to an episode skips both the `ani-cli` spawn and the upstream link-discovery round trip. Entries are invalidated only when a cached URL fails on use; allmanga's slugs rotate every few weeks, so the layer self-heals via the silent retry path rather than a wall-clock TTL.

The **availability TTL branches on Kitsu's `status` field**: shows airing weekly need a 24-hour window so a new episode surfaces within a day, but finished shows can hold for 30 days. Unknown / missing status falls back to the short (24h) window — a stale "no episode 1161 yet" is much worse than re-probing too eagerly.

## Embedded playback

Playback happens inside the desktop window — not in a detached `mpv` process. Implementation:

- `<video>` element receives the master.m3u8 URL from the local proxy.
- `hls.js` handles HLS streams. mp4 streams (some providers) play natively via `<video src=...>`.
- Subtitles render via `<track kind="subtitles">` from the proxied `.vtt` URL ani-cli already extracts.
- Quality switching maps to hls.js's `currentLevel` for HLS, or re-resolution for mp4.

An "Open in external player" button on the player chrome launches the user's `mpv` (or platform default) with the same arguments `ani-cli` would have used. This is a user choice, never an automatic fallback — silent fallback would be confusing.

### Skip OP / ED via aniskip

The player surfaces "Skip Opening" / "Skip Outro" buttons during their respective intervals. The skip times come from [aniskip.com](https://aniskip.com)'s community-submitted database, keyed by MyAnimeList id rather than allanime or Kitsu. The backend bridges Kitsu → MAL using Kitsu's mappings endpoint, then asks aniskip for `(mal_id, episode)` skip intervals and caches the response for 7 days (skip times stabilize quickly once submitted). When auto-skip is enabled in settings, the player jumps the playhead past the interval automatically; otherwise it just shows the button.

### Persistent Picture-in-Picture across navigation

The Fullscreen and Picture-in-Picture APIs both bind to a specific `HTMLVideoElement` instance: removing the element from the DOM closes the PiP window. SvelteKit destroys page components on route change, which would otherwise kill PiP every time the user clicked away from the player.

The app sidesteps that by parking the `<video>` element in a hidden 1×1 host attached to `document.body` — body lives outside Svelte's reactive tree, so the element survives any number of route changes. The play page is a "controller" for that singleton: on mount it moves the element into its player frame; on destroy it moves it back to the hidden host. PiP keeps drawing throughout.

Navigation away from the player branches three ways:

1. **Episode swap on the same show** — the singleton stays attached to the play frame; the new page's load effect swaps its `src` in place. No PiP, no teardown.
2. **Different route or different show**, auto-PiP enabled (default) — the page calls `requestPictureInPicture()` from the navigation hook, the floating window appears, the user keeps watching while they browse. Closing the PiP window navigates back to `/play/[id]` so the stream surfaces inline again.
3. **Different route or different show**, auto-PiP disabled — the page pauses the singleton instead of requesting PiP. Without an explicit pause the off-screen element would keep streaming audio in the background.

Clicking back into the same episode reuses the live session: the play page's load effect detects that the singleton already has the right `src` loaded and skips re-attaching, so playback resumes at its current timestamp instead of restarting from zero.

## User settings

User-editable settings live in `$XDG_CONFIG_HOME/ani-gui/config.toml`. The Settings page reads/writes via the backend (`GET / PUT /api/settings`); changes apply immediately to the surfaces that observe them. Available fields:

| Field | Default | Effect |
|---|---|---|
| `mode` | `"sub"` | Audio mode for new play / download calls — `"sub"` or `"dub"`. |
| `quality` | `"best"` | Quality bucket — `"best"`, `"1080"`, `"720"`, `"480"`, `"worst"`. |
| `locale` | `"en"` | UI locale (the four MVP locales — see [`i18n.md`](./i18n.md)). |
| `external_player` | `"mpv"` | Command launched by "Open in external player". |
| `image_cache_cap_mb` | `500` | Cap for the on-disk image cache; LRU evicts above this. |
| `auto_play_next` | `false` | When the current episode ends, automatically resolve and play the next one. |
| `auto_skip_op` | `false` | When aniskip has an OP interval, jump past it automatically. |
| `auto_skip_ed` | `false` | Same as above, for the ED. |
| `use_custom_player_controls` | `false` | Replace the browser's native controls with the in-app two-row bar. The native bar gives free PiP/captions menus; the custom bar keeps the Skip OP/ED button visible during fullscreen. |
| `disable_auto_pip_on_leave` | `false` | When set, navigating away from the player pauses playback instead of entering PiP. |
| `download_bottom_bar_enabled` | `true` | Show the per-download progress dock at the bottom of the window when downloads are active. |

## Localization

Four MVP locales: English (`en`), Brazilian Portuguese (`pt-BR`), Latin American Spanish (`es-419`), Russian (`ru`). The set was chosen for free-content market fit, not language-coverage prestige. Phase-2 candidates listed in `docs/i18n.md`.

The backend never returns localized text. Errors are stable keys (`error.scraper.timeout`, `error.search.no_results`, etc.); the frontend resolves them via Paraglide. Anime titles themselves are not translated by the app — they come from Kitsu/AniList per a user-chosen title-language preference.

## Why a separate CLI still exists

The `ani-cli` script at the repository root is a fully functional, separately installable artifact. The GUI does not replace it. The two coexist:

- Users who want a terminal flow run `ani-cli` directly. The GUI is not installed and not required.
- Users who want a graphical experience install the desktop bundle.

The script is mergeable from upstream `pystardust/ani-cli` without conflict because the GUI lives entirely under `gui/` and only invokes the script as a subprocess. The single carried patch is a `__ANI_CLI_LIB__` source guard added near the bottom of the script for testability.

## Design direction (UI as a first-class surface)

The pivot from CLI to GUI is positioned as a premium-experience product: the UI is the differentiator. The design direction explicitly rejects generic AI aesthetics and embraces:

- Dynamic per-anime theming, with accent colors extracted from `coverImage.color`.
- Editorial typography pairing — a display face for hero titles, a clean body face for paragraphs, oversized tabular numerals for episode numbers.
- Motion as structure: elastic-eased carousels, parallax-on-hover cards, shared-element page transitions (poster card morphs into the detail-page poster), theater-dim into playback.
- Subtle anime motifs: manga-page-inspired dividers, oversized episode numerals, occasional Japanese typography accents — restrained, not cosplay.
- A player chrome closer to Apple TV+ than to VLC: minimal, autohides cleanly.

`tests/arch/i18n.sh` enforces that no `.svelte` file ships with hardcoded English. The wider design guard rails are documented in `AGENTS.md` §7.
