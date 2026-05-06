# Development

This page covers the dev environment, the build pipeline, and debugging tips. For architecture, read `architecture.md`. For test discipline, read `testing.md`.

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` (currently `1.82`) | backend + Tauri shell |
| Node | 20+ | frontend toolchain |
| pnpm | 9+ | frontend package manager |
| Tauri prerequisites | per platform | webview + bundling |
| `bats-core`, `bats-mock`, `bats-assert`, `bats-file` | pinned in `tests/bash/helpers/install-bats.sh` | bash tests |
| `shellcheck`, `shfmt` | latest stable | bash linters |
| `mpv` | any | optional escape-hatch player |

Tauri's per-platform prerequisites: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Linux you need `webkit2gtk-4.1`, `librsvg2`, `libappindicator3`, and `pkg-config`.

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
# Rust backend + Tauri shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install -y libwebkit2gtk-4.1-dev librsvg2-dev \
  libayatana-appindicator3-dev libsoup-3.0-dev \
  build-essential libssl-dev pkg-config \
  patchelf
```

`patchelf` is required by `linuxdeploy-plugin-gstreamer` when
`cargo tauri build --bundles appimage` packs the GStreamer plugins
the embedded webview needs for media playback.

```sh
# Frontend (Node + pnpm via nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
# (open a new shell, or `source ~/.bashrc`)
nvm install 20 && nvm use 20
corepack enable && corepack prepare pnpm@latest --activate
```

```sh
# End-to-end tests (optional locally — CI runs them anyway)
sudo apt install -y webkit2gtk-driver xvfb
cargo install tauri-driver --locked
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

# Frontend
cd gui/frontend && pnpm install && cd ../..

# Verify Rust toolchain
cd gui/src-tauri && cargo --version && cd ../..
```

### Other distros

Mostly same packages, different package manager. PRs welcome to add Fedora / Arch / openSUSE recipes here. The Tauri prereqs page covers each:
<https://v2.tauri.app/start/prerequisites/>.

## Dev loop

Two terminals:

```sh
# Terminal 1 — frontend dev server with HMR
cd gui/frontend && pnpm dev

# Terminal 2 — Tauri shell pointing at the dev server
cd gui/src-tauri && cargo tauri dev
```

`tauri.conf.json` has `build.devUrl = "http://localhost:5173"`, so Tauri opens the webview pointed at Vite. Frontend changes hot-reload. Rust changes recompile on save.

## Build for distribution

```sh
cd gui/src-tauri
cargo tauri build              # builds for the host platform
```

Artifacts land in `gui/src-tauri/target/release/bundle/`. CI builds all five targets on every release tag:

| Target | Runner | Output |
|---|---|---|
| AppImage | `ubuntu-22.04` | `*.AppImage` |
| `.deb` | `ubuntu-22.04` | `*.deb` |
| Flatpak | `ubuntu-22.04` (flatpak-builder) | `*.flatpak` |
| `.dmg` (Intel + Apple Silicon) | `macos-13`, `macos-14` | `*.dmg` |
| `.msi` | `windows-latest` | `*.msi` |

## Logging and debugging

The backend uses [`tracing`](https://docs.rs/tracing). Adjust verbosity:

```sh
RUST_LOG=ani_gui=debug,axum=info cargo tauri dev
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
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Linux-only workaround for black-screen on some Mesa drivers |

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
By design — during M0/M1 development, opening `http://localhost:5173` in any browser shows the same UI as the Tauri webview. Useful for fast iteration, but production builds always run inside Tauri because the streaming proxy + native window matter for the shipped product.
