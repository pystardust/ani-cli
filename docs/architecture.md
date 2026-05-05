# Architecture

`ani-gui` is a desktop app that lets you browse and watch anime through a graphical interface. Under the hood it reuses the [`ani-cli`](https://github.com/pystardust/ani-cli) Bash scraper unmodified and adds a Rust + SvelteKit frontend on top of it.

## What gets shipped

A single-window desktop application. Linux: AppImage, `.deb`, Flatpak. macOS: `.dmg`. Windows: `.msi`. The user double-clicks an icon and gets a native window. There is no URL to visit, no port to remember, no internet-reachable service.

## Why there is a "backend"

The app talks to three things a browser tab cannot reach on its own:

1. The vendored `ani-cli` script — needs a subprocess.
2. The shared history file at `$XDG_STATE_HOME/ani-cli/ani-hsts` — needs filesystem access.
3. Anime stream CDNs — require a `Referer:` header that browser fetch APIs cannot set, and serve segments without permissive CORS.

So the app embeds a Rust backend, bound to `127.0.0.1` on a kernel-assigned port, that orchestrates these pieces. It is a localhost daemon shipped inside the Tauri bundle, not a server that anyone else can reach.

## Components

```
 ┌────────────────────────────────────────────────────────────┐
 │             ani-gui — Tauri bundle (one process)           │
 │                                                            │
 │  ┌──────────────────┐      IPC      ┌──────────────────┐   │
 │  │  Frontend (UI)   │ ────────────► │   Backend (Rust) │   │
 │  │  SvelteKit SPA   │ ◄──────────── │                  │   │
 │  │  + hls.js        │               │                  │   │
 │  └────────┬─────────┘               └─────┬────────────┘   │
 │           │                               │                │
 │           │ <video src="http://127.0.0.1: ├──► ani-cli     │
 │           │  PORT/s/<token>/...">         │   subprocess   │
 │           │                               │                │
 │           │  bytes piped through proxy    ├──► Kitsu (REST)│
 │           └──────────────────────────────►│   AniList      │
 │                  Referer + CORS           │   (GraphQL)    │
 │                                           │                │
 │                                           ├──► history     │
 │                                           │   (TSV file)   │
 │                                           │                │
 │                                           └──► SQLite +    │
 │                                              image cache   │
 └────────────────────────────────────────────────────────────┘

 sibling: pystardust/ani-cli (vendored, untouched)
```

Three layers, in lockstep:

- **Frontend** — SvelteKit static SPA running inside Tauri's webview. Renders the discovery surface, search results, detail pages, and the embedded player (`<video>` + hls.js). Stateless beyond UI state; talks only to the backend.
- **Backend** — Rust crate inside `gui/src-tauri/`. Spawns `ani-cli` as a subprocess, fetches metadata from Kitsu/AniList, reads/writes the shared history file, runs a streaming proxy on a localhost port, and exposes a typed IPC surface to the frontend.
- **External processes** — `ani-cli` (the script) and optionally `mpv` for the "Open in external player" escape hatch.

## Data flow: searching and playing an episode

1. The user types a query into the search bar.
2. The frontend invokes the `search` command. The backend spawns `ani-cli -S <n>` with `ANI_CLI_PLAYER=debug` and parses the result lines into typed `SearchResult` records.
3. The user picks a result; the frontend invokes `episodes_list`. The backend spawns `ani-cli` again with the chosen ID and returns the episode list.
4. The user clicks an episode. The frontend invokes `resolve_stream`. The backend spawns `ani-cli` once more, this time with `-e <ep>` and `ANI_CLI_PLAYER=debug`, parses the resolved stream URL plus its `Referer` requirement and any subtitle `.vtt` URL.
5. The backend creates a `StreamSession` (UUID, upstream URL, referer, expiry), stores it in memory, and returns a token to the frontend.
6. The frontend mounts `<video>` and points hls.js at `http://127.0.0.1:<port>/s/<token>/master.m3u8`.
7. The streaming proxy fetches the upstream master playlist with the correct `Referer:` header, parses it with `m3u8-rs`, and rewrites every variant + segment URI to flow back through itself with HMAC-signed sub-tokens. CORS headers are added so hls.js inside the webview can consume the rewritten manifest without preflight blocks.
8. Subsequent segment requests follow the same path: hls.js asks the proxy, the proxy asks the upstream with the `Referer:`, bytes stream back.

## Discovery (landing page)

The landing page shows four rows: **Trending Now**, **Popular This Season**, **Top Rated**, **Recently Released**.

- **Trending Now** is fetched from AniList's GraphQL endpoint (`Page.media(sort: TRENDING_DESC)`). AniList's trending sort tracks current weekly popularity, which Kitsu's `userCount` cannot match.
- **Popular This Season**, **Top Rated**, and **Recently Released** are fetched from Kitsu (REST/JSON:API). Kitsu's posters and banners (sizes verified at build time) are sufficient for these views.
- Both APIs are hit only when cache misses; cache is SQLite (`$XDG_CACHE_HOME/ani-gui/meta.db`) with TTLs from 1 hour (trending) up to 30 days (title-match cache).

When a user clicks a discovery card, the backend resolves its title against `ani-cli` (search by every available alias from the metadata API: English, Romaji, Native, synonyms) and falls into the same playback flow.

## Caching

| Asset | Storage | TTL |
|---|---|---|
| AniList trending row | SQLite `meta_cache` | 1 hour |
| Kitsu seasonal / top / recent | SQLite `meta_cache` | 6 hours |
| Per-anime metadata (`/anime/:id`) | SQLite `meta_cache` | 7 days |
| Title matches (Kitsu/AniList → allanime ID) | SQLite `title_match` | 30 days |
| Poster + banner image bytes | Filesystem `images/<shard>/<hash>.<ext>` | LRU, capped at 500 MB |

Image bytes never live in SQLite; they're filesystem-keyed by `sha256(url)[..16]`, sharded two-deep to avoid huge flat directories.

## Embedded playback

Playback happens inside the GUI window — not in a detached `mpv` process. Implementation:

- `<video>` element receives the master.m3u8 URL from the local proxy.
- `hls.js` handles HLS streams. mp4 streams (some providers) play natively via `<video src=...>`.
- Subtitles render via `<track kind="subtitles">` from the proxied `.vtt` URL ani-cli already extracts.
- Quality switching maps to hls.js's `currentLevel` for HLS, or re-resolution for mp4.

An "Open in external player" button on the player chrome launches the user's `mpv` (or platform default) with the same arguments `ani-cli` would have used. This is a user choice, never an automatic fallback — silent fallback would be confusing.

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
