# ani-gui frontend

SvelteKit static SPA that runs inside the Tauri 2 webview. Talks to the
Rust backend through `@tauri-apps/api/core` IPC commands and to the
local stream proxy through `<video>` + `hls.js`.

## Quick start

```sh
pnpm install
pnpm run dev          # vite dev server on http://127.0.0.1:5173
pnpm run check        # svelte-check + tsc
pnpm run lint         # prettier --check + eslint
pnpm run test         # vitest run
```

The full app boots with `cargo tauri dev` from `../src-tauri/`, which
invokes `pnpm run dev` for you via `beforeDevCommand`.

## Layout

- `src/routes/` — SvelteKit pages
- `src/lib/api.ts` — typed wrappers around Tauri commands
- `src/lib/player/` — `<video>` + hls.js plumbing (added in M1.5e)

## Design

Visual design is intentionally bare in M1.5. The full design pass —
typography, color, motion, layout, anime-aware theming — lands in M3
through the `frontend-design` Anthropic skill with sub-agent review.
See `../../docs/architecture.md` for the design direction.
