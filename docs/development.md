# Development

This page covers the dev environment, the build pipeline, and debugging tips. For architecture, read `architecture.md`. For test discipline, read `testing.md`.

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` | Rust sidecar backend |
| Node | 20+ | renderer + Electron shell |
| pnpm | 9+ | package manager |
| `bats-core`, `bats-mock`, `bats-assert`, `bats-file` | pinned in `tests/bash/helpers/install-bats.sh` | bash tests |
| `shellcheck`, `shfmt` | latest stable | bash linters |
| `mpv` | any | optional escape-hatch player |

## First-time setup

### Ubuntu / Debian (copy-paste)

Group these by what subsystem you intend to work on. You only need a group's tools when you're building or testing in that subsystem.

```sh
# Bash subsystem (the vendored CLI script + its test suite)
sudo apt install -y shellcheck kcov
# shfmt is not in 24.04 apt; install the static binary:
sudo curl -sSL -o /usr/local/bin/shfmt \
  https://github.com/mvdan/sh/releases/download/v3.10.0/shfmt_v3.10.0_linux_amd64 \
  && sudo chmod +x /usr/local/bin/shfmt
```

```sh
# Rust sidecar backend
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install -y build-essential libssl-dev pkg-config
```

```sh
# Renderer + Electron shell (Node + pnpm via nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
# (open a new shell, or `source ~/.bashrc`)
nvm install 20 && nvm use 20
corepack enable && corepack prepare pnpm@latest --activate
```

```sh
# Quality of life (optional)
sudo apt install -y mpv jq ripgrep
```

### Then in the repo

```sh
git clone git@github.com:JoaoPucci/ani-gui.git
cd ani-gui

# Bash test toolchain (vendored bats + plugins at pinned tags)
./tests/bash/helpers/install-bats.sh

# Frontend + Electron deps
cd gui/frontend && pnpm install && cd ../..
cd gui/electron  && pnpm install && cd ../..

# Verify Rust toolchain
cd gui/backend   && cargo --version && cd ../..
```

### Other distros

Mostly same packages, different package manager. PRs welcome to add Fedora / Arch / openSUSE recipes.

## Dev loop

Three terminals:

```sh
# Terminal 1 — Vite dev server with HMR
cd gui/frontend && pnpm dev          # http://localhost:5173

# Terminal 2 — build the Rust sidecar (one-shot per Rust change)
cd gui/backend && cargo build --bin ani-gui-backend

# Terminal 3 — Electron shell (spawns the sidecar, points at Vite)
cd gui/electron && pnpm dev
```

The Electron main process resolves the backend binary (`gui/backend/target/debug/ani-gui-backend`), spawns it, and parses its stdout `ANI_GUI_LISTENING <url>` handshake to discover the loopback port. The renderer reads that URL from `window.aniGui.apiBase` (set by the Electron preload script) and uses it for every `fetch()` call.

## Build for distribution

```sh
cd gui/electron
pnpm package          # AppImage only — fast iteration
pnpm package:release  # AppImage + .deb
```

Artifacts land in `gui/electron/dist/`. CI builds all targets on every release tag:

| Target | Runner | Output |
|---|---|---|
| AppImage | `ubuntu-22.04` | `*.AppImage` |
| `.deb` | `ubuntu-22.04` | `*.deb` |
| Flatpak | `ubuntu-22.04` (flatpak-builder) | `*.flatpak` |
| `.dmg` (Intel + Apple Silicon) | `macos-13`, `macos-14` | `*.dmg` |
| `.msi` | `windows-latest` | `*.msi` |

## Logging and debugging

The backend uses [`tracing`](https://docs.rs/tracing). Adjust verbosity by setting `RUST_LOG` before launching the backend (or the Electron shell that spawns it):

```sh
RUST_LOG=ani_gui=debug,axum=info pnpm --dir gui/electron dev
```

Logs also tee to `$XDG_DATA_HOME/ani-gui/logs/ani-gui.log` (daily rotation, 7-day retention).

The streaming proxy port is logged at startup:

```
INFO ani_gui::proxy: stream proxy listening on 127.0.0.1:42337
```

Use it to inspect proxied requests with `curl`:

```sh
curl -sI http://127.0.0.1:42337/healthz
```

## Useful environment variables

| Variable | Purpose |
|---|---|
| `RUST_LOG` | tracing filter |
| `ANI_CLI_PLAYER` | propagated when invoking ani-cli; the GUI defaults to `debug` |
| `ANI_CLI_HIST_DIR` | shared with the CLI; the GUI reads/writes the same `ani-hsts` |
| `ANI_GUI_UPSTREAM_BASE` | dev/test only; redirects `meta_http` to a wiremock instance |
| `VITE_ANI_GUI_API_BASE` | browser-only dev: point the Vite renderer at a separately-running backend |

## Code style

- **Rust**: `cargo fmt` (settings in `rustfmt.toml`); `cargo clippy -D warnings` enforced by CI.
- **TS / Svelte**: `prettier` for formatting; `eslint` (svelte plugin + custom rules) for behavior.
- **Bash**: `shfmt -i 4 -ci -d` matches upstream `pystardust/ani-cli`. Apply identically to the CLI script and `tests/bash/`.

The project's hard rule on the CLI script: it is vendored from upstream and must never be reformatted. See `AGENTS.md` §3.

## Frequently asked

**Why doesn't an `mpv` window pop up when I play something?**
The GUI plays inside the window using hls.js + a local stream proxy. `mpv` is only launched if you click "Open in external player".

**Why does `ani-gui` look at `~/.local/state/ani-cli/ani-hsts`?**
History is shared between the CLI and the GUI. Watch in one, continue in the other.

**Why does the dev server work in a browser tab too?**
By design — opening `http://localhost:5173` in any browser shows the same UI the Electron renderer loads. Useful for fast iteration. Production builds always run inside Electron because the streaming proxy + native window matter for the shipped product; standalone-browser dev only works against a separately-running backend (set `VITE_ANI_GUI_API_BASE` to its loopback URL).
