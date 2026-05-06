# ani-gui frontend

SvelteKit static SPA that runs inside the Electron renderer. Talks to
the Rust sidecar (`ani-gui-backend`) over plain HTTP via `fetch()`,
and to the local stream proxy through `<video>` + `hls.js`.

## Quick start

```sh
pnpm install
pnpm run dev          # vite dev server on http://127.0.0.1:5173
pnpm run check        # svelte-check + tsc
pnpm run lint         # prettier --check + eslint
pnpm run test         # vitest run
```

The full app boots from `../electron/` — see `gui/electron/README.md`
for the three-process dev loop (Vite + backend bin + Electron shell).

## Layout

- `src/routes/` — SvelteKit pages
- `src/lib/api.ts` — typed `fetch()` wrappers around the backend's
  `/api/*` routes; reads its base URL from `window.aniGui.apiBase`
  (set by Electron's preload script).
- `src/lib/player/` — `<video>` + hls.js plumbing.

## Design

See `../../docs/architecture.md` for the design direction —
typography, motion, anime-aware theming, and the brand mark.
