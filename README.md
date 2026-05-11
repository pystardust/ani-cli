<h1 align="center">ani-gui</h1>

<p align="center">
  <em>A desktop app for browsing and watching anime.</em>
</p>

<p align="center">
  <img width="2751" height="1300" alt="image" src="https://github.com/user-attachments/assets/ee2e3d80-01e8-46cb-afa0-a132cd3e3273" />
</p>

ani-gui is a graphical front-end for [pystardust/ani-cli](https://github.com/pystardust/ani-cli). It embeds the upstream Bash scraper unmodified and wraps it in a Rust + SvelteKit desktop application — discovery, search, an embedded player, downloads, persistent watch history, Picture-in-Picture, and OP/ED skip on top of the same scraping engine.

The CLI still exists. The GUI does not replace it; the two share the script and coexist in this repository. See [`docs/architecture.md`](./docs/architecture.md) for the full picture.

## What you get

- **Discovery landing page** — Trending, Popular This Season, Top Rated, and Recently Released rows, mixing AniList trending data with Kitsu metadata.
- **Search** — full-text against Kitsu, with instant results as you type.
- **Detail view per show** — synopsis, episode list with thumbnails, similar-titles strip.
- **Embedded HLS / MP4 player** — built in, no separate `mpv` window required. Quality switching, native or custom controls.
- **Subtitles** — rendered via `<track kind="subtitles">` from the same `.vtt` upstream provides.
- **Skip OP / ED via aniskip** — community-submitted intervals; one-click button or fully automatic via a settings toggle.
- **Picture-in-Picture across navigation** — keep watching while you browse the rest of the app.
- **Background prefetch** — adjacent episodes warm in advance so episode-to-episode boundaries don't stutter.
- **Downloads** — per-episode or ranged, with a progress dock at the bottom of the window. ffmpeg + aria2c are bundled where required.
- **Persistent history** — shared with the CLI. The history file is `$XDG_STATE_HOME/ani-cli/ani-hsts`, the same path `ani-cli` writes to, so your progress survives a switch between the two.
- **External-player escape hatch** — one click to launch your configured player (`mpv` by default; VLC / IINA / a custom command also supported) with the resolved stream URL.
- **Localised UI** — English, Brazilian Portuguese, Latin American Spanish, Russian.
- **No telemetry** — the app makes only the requests required to fetch metadata and stream chosen episodes. No phone-home, no analytics, no account.

## Install

ani-gui is distributed as a desktop bundle. The bundled script is updated automatically on launch (see *Self-update* below); a separate `ani-cli` install is **not** required.

Platform support tiers:

| Tier | Platform | Status |
|---|---|---|
| 1 | Linux | Actively tested on Ubuntu. Other distros work via AppImage. |
| 2 | Windows | Most features verified end-to-end. Edge cases may surface. |
| 3 | macOS | Untested. Builds the same way; should work — please file an issue if it doesn't. |

<details>
<summary><strong>Linux</strong> — tier 1 (tested on Ubuntu)</summary>

- **AppImage** — download from the [releases page](https://github.com/JoaoPucci/ani-gui/releases), `chmod +x`, double-click. The bundle launches with Chromium's setuid sandbox disabled (AppImage's read-only FUSE mount can't carry the SUID bit `chrome-sandbox` requires); the localhost-only architecture means the sandbox isn't load-bearing for the threat model. If you'd rather keep the sandbox, install the `.deb` instead.
- **Debian / Ubuntu (`.deb`)** — `sudo apt install ./ani-gui_<version>_amd64.deb`. apt pulls in the recommended `ffmpeg` package (needed for the download feature) along the way; the post-install script sets the `chrome-sandbox` SUID bit Electron needs, so the sandbox stays on. `sudo dpkg -i …` still works but won't auto-install ffmpeg — drop into `apt --fix-broken install` or run `sudo apt install ffmpeg` separately if you used dpkg directly.

</details>

<details>
<summary><strong>Windows</strong> — tier 2 (most functions tested)</summary>

NSIS installer (`.exe`). Run it; it installs per-user by default and creates Start menu and desktop shortcuts.

ani-gui drives the upstream `ani-cli` Bash script via `bash`, which on Windows ships as part of [Git for Windows](https://gitforwindows.org/). If Git for Windows isn't installed when you launch the app, you'll see a dialog with a one-click link to its download page. The installer will fetch ffmpeg automatically the first time it runs (~80 MB) so the download feature works out of the box; aria2c and fzf are bundled directly. The ffmpeg fetch runs even when you already have ffmpeg installed via a per-user package manager (scoop, winget user-scope) — the installer's elevated context doesn't see per-user PATH entries, and the bundled copy is what the app uses at runtime in either case.

</details>

<details>
<summary><strong>macOS</strong> — untested</summary>

A `.dmg` is produced by the same `electron-builder` config and should install via the standard drag-into-Applications flow. macOS isn't part of the regular acceptance pass, so if you hit a problem please [open an issue](https://github.com/JoaoPucci/ani-gui/issues) — the app is shipped for it but unverified.

</details>

<details>
<summary><strong>Build from source</strong> — any platform</summary>

See [`docs/development.md`](./docs/development.md). Short version: Rust toolchain, Node 24+, pnpm, then:

```bash
cd gui/frontend && pnpm install
cd ../electron && pnpm install
pnpm run package          # Linux AppImage + .deb
pnpm run package:win      # Windows NSIS installer (cross-builds on Linux)
```

</details>

## First run

On first launch the app:

1. Spawns the Rust sidecar on a kernel-assigned localhost port (no fixed port, no internet-reachable service).
2. Materialises the bundled `ani-cli` script to `$XDG_CACHE_HOME/ani-gui/ani-cli` so it can be patched in place by `-U`.
3. Runs `bash ani-cli -U` in the background to pick up any same-day upstream hotfixes.
4. Loads the discovery surface.

After that, click anything that looks clickable. The app routes the click through Kitsu / AniList for metadata, `ani-cli` for the actual stream resolution, and the embedded player for playback.

## Configuration

User settings live in `$XDG_CONFIG_HOME/ani-gui/config.toml`. The Settings page exposes everything you'd normally edit:

- audio mode (`sub` / `dub`) and quality (`best`, `1080`, `720`, `480`, `worst`)
- UI locale
- external-player kind and command
- image-cache size cap
- auto-play next episode
- auto-skip OP / ED
- custom-vs-native player controls
- whether to enter PiP automatically when you navigate away from a playing video
- whether to keep `ani-cli` self-updating on launch

Full table with defaults and effects is in [`docs/architecture.md`](./docs/architecture.md#user-settings).

### Self-update of the bundled scraper

Allmanga (the catalogue `ani-cli` scrapes) changes its API often, and upstream `pystardust/ani-cli` ships hotfixes daily. The bundled snapshot in your install would go stale within a week.

ani-gui handles this for you: on every launch a background task runs `bash <cached-ani-cli> -U`, captures the outcome, and persists the last few attempts. The app itself isn't blocked by the update — startup proceeds normally; the script is patched in place by the next time you trigger a search or a play.

The flow is gated by the **Auto-update ani-cli** setting (default ON). When it's off, the bundle just keeps using whatever script is in your cache, indefinitely. The latest update outcome is visible on the **/diagnostics** page.

## Privacy

ani-gui is a localhost daemon plus a renderer — there is no server-side component. The only outbound traffic the app makes:

- Kitsu and AniList for metadata.
- aniskip for OP/ED intervals.
- The streaming providers `ani-cli` already talks to, plus their referer-required CDNs (proxied through the localhost binary so the renderer can `fetch()` past CORS).
- Image hosts for poster / banner thumbnails.

No telemetry, no analytics, no account, no remote control. The localhost listener binds to `127.0.0.1` only, on a kernel-assigned port that changes every launch.

## Troubleshooting

**"missing bash.exe (install Git for Windows)"** on first launch — install [Git for Windows](https://gitforwindows.org/) and relaunch. ani-gui locates `bash.exe` automatically once it's on PATH or in the standard install location.

**"Download needs ffmpeg"** dialog — the installer normally fetches ffmpeg during setup (~80 MB). If your install was offline, re-run the installer with internet, or drop `ffmpeg.exe` into `<install dir>\resources\bin\` manually.

**"Couldn't launch <player>"** when you click *Open in external* — the configured external player isn't on PATH. Open Settings and either pick a different player kind or browse to the binary's full path.

**No results / playback fails right away** — the most common cause is upstream catalogue drift. The bundled scraper auto-updates on launch; if it's too soon after a fresh install, give it a minute and retry, or visit the **/diagnostics** page to see the last update attempt.

**Stale "no episode N yet" on a currently-airing show** — availability is cached for 24 hours on ongoing shows. The cache self-invalidates the next time the worker runs; the diagnostics page shows the cached row's age.

## How it works

A two-line summary: a Rust sidecar embedded inside an Electron shell speaks to Kitsu / AniList / aniskip and spawns `ani-cli` as a subprocess for stream resolution. A streaming proxy in the sidecar adds the right `Referer:` headers and rewrites HLS playlists so the embedded `<video>` element can play upstream content without CORS or referer issues. SQLite caches metadata; the filesystem caches images.

For the long version — diagrams, cache TTLs, the title-resolution bridge, the PiP architecture — see [`docs/architecture.md`](./docs/architecture.md), [`docs/title-resolution.md`](./docs/title-resolution.md), and the rest of [`docs/`](./docs/).

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) and [`docs/development.md`](./docs/development.md). The repository carries one upstream patch (a single source-guard line in `ani-cli` for testability) and otherwise mirrors `pystardust/ani-cli` so script-side fixes flow in without conflict.

## Acknowledgements

ani-gui only exists because of the projects it builds on:

- **[pystardust/ani-cli](https://github.com/pystardust/ani-cli)** — the Bash scraper that does the actual stream resolution. ani-gui ships the script unmodified.
- **[Kitsu](https://kitsu.io/)** and **[AniList](https://anilist.co/)** for the metadata, posters, and trending data behind the discovery surface.
- **[aniskip](https://aniskip.com/)** for the community-submitted OP/ED intervals.
- **[hls.js](https://github.com/video-dev/hls.js/)** for the HLS playback inside the embedded player.

## Disclaimer

ani-gui is a tool. Like any tool, the responsibility for how it's used lies with the user. The app makes no claim on the content it surfaces — it talks to the same providers you'd reach in a browser and routes their output through your machine. The full project disclaimer applies: see [`disclaimer.md`](./disclaimer.md).

## License

[GPL-3.0](./LICENSE), inheriting from upstream `pystardust/ani-cli`.
