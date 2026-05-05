# Testing

This project is **strictly TDD**. Every code change starts with a failing test, including the first commit retrofitting tests onto the existing CLI script.

## The pyramid

```
         ┌─────────────────────┐
         │   Mutation (deferred)│  cargo-mutants + stryker, nightly,
         └─────────────────────┘  not gating
       ┌───────────────────────────┐
       │ Architectural invariants  │  tests/arch/*.sh — boundary,
       └───────────────────────────┘  i18n, deps, capabilities
     ┌─────────────────────────────────┐
     │           Property              │  proptest (Rust), fast-check (TS),
     └─────────────────────────────────┘  bash generator harness
   ┌──────────────────────────────────────┐
   │              End-to-end              │  Playwright + tauri-driver,
   └──────────────────────────────────────┘  ≤5 hermetic scenarios
 ┌──────────────────────────────────────────┐
 │              Acceptance                  │  bats `run`, cargo integration,
 └──────────────────────────────────────────┘  vitest + MSW
┌──────────────────────────────────────────────┐
│                  Unit                        │  bats-core, cargo test,
└──────────────────────────────────────────────┘  vitest, colocated *.test.ts
```

## Test layout

```
tests/
├── bash/                  # bats-core suites for ani-cli
│   ├── helpers/           # loader, curl/mpv stubs, common assertions
│   ├── unit/              # one .bats per pure function
│   ├── network/           # curl-mocked tests
│   ├── subprocess/        # mpv/ffmpeg/aria2c stubs
│   ├── acceptance/        # full-CLI-run scenarios
│   └── property/          # generator harness for pure functions
├── fixtures/              # shared goldens (bash + rust + ts)
│   ├── allanime/          # GraphQL responses, tobeparsed blobs
│   ├── kitsu/             # JSON:API responses
│   ├── anilist/           # GraphQL responses
│   ├── m3u8/              # master + media playlists, edge cases
│   └── history/           # ani-hsts samples
└── arch/                  # cross-cutting architectural invariants
    ├── boundaries.sh
    ├── i18n.sh
    ├── bash_portability.sh
    ├── capabilities.sh
    └── deps.toml          # cargo-deny config

gui/src-tauri/
├── src/                   # #[cfg(test)] mod tests inline
├── tests/                 # cargo integration tests (acceptance)
└── proptests/             # proptest-only suites

gui/frontend/
├── src/                   # *.test.ts colocated with units
├── tests/acceptance/      # vitest + MSW
└── e2e/                   # Playwright + tauri-driver
```

## Running tests locally

```sh
# Bash (CLI retrofit + new bash code)
tests/bash/helpers/install-bats.sh    # one-time, pins bats-core + plugins
bats tests/bash/

# Rust backend
cd gui/src-tauri && cargo test --workspace
cd gui/src-tauri && cargo test --test proptests

# Frontend
cd gui/frontend && pnpm test
cd gui/frontend && pnpm test:acceptance

# E2E (requires Tauri dev build)
cd gui/frontend && pnpm test:e2e

# Architectural invariants (always fast)
bash tests/arch/run-all.sh
```

## Coverage targets

Layer-specific. CI fails on regression below the baseline in `coverage-baseline.json`, not on absolute floors.

| Layer | Tool | Line | Branch |
|---|---|---|---|
| Bash pure functions | bashcov / kcov | 95% | 90% |
| Bash network/subprocess | bashcov | 70% | — |
| Rust core (proxy, cache, anicli, history) | `cargo llvm-cov` | 85% | 75% |
| Rust glue (Tauri commands) | `cargo llvm-cov` | 60% | — |
| Frontend lib/stores | vitest + c8 | 80% | — |
| Frontend components | vitest + c8 | 50% | — |
| E2E | scenario count | ≥5 | — |

## CI gates

Every PR runs all gating workflows; merge blocks until they're green:

| Workflow | Triggers | Gating |
|---|---|---|
| `ani-cli.yml` (upstream-aligned) | PR touches `**ani-cli` | yes |
| `bash.yml` | PR touches `tests/bash/**` or `ani-cli` | yes |
| `rust.yml` | PR touches `gui/src-tauri/**` or `Cargo.lock` | yes |
| `frontend.yml` | PR touches `gui/frontend/**` | yes |
| `arch.yml` | always | yes |
| `e2e.yml` | PR touches `gui/frontend/**` or `gui/src-tauri/src/proxy/**` | yes (path-conditional) |
| `mutation.yml` | nightly cron + manual dispatch | no (informational) |

## Fixture management

`tests/fixtures/` is the single source of truth for golden data, shared across all test layers.

- Each subdirectory has a `MANIFEST.json` listing every fixture's source URL, capture date, and SHA-256.
- Fixtures over 1 MB live in git-LFS.
- Refresh via `make fixtures-refresh`, which re-records against live APIs and writes a diff report. The diff is reviewed in the PR.

## Property-based testing

Targets:

- **Rust**: `select_quality` invariants, m3u8 rewriter idempotency, URL token roundtrip, history file parse/serialize roundtrip, cache TTL monotonicity.
- **TypeScript**: episode range parser (`"5-7"`, `"-1"`, `"5 6"`), search query sanitizer idempotency, Paraglide message-key existence in every locale.
- **Bash**: `nth`, `select_quality`, `b64url_to_hex` — emulated property tests via a 200-iteration generator harness in `tests/bash/property/` since bats has no native shrinking.

## Architectural invariants

Cheap grep / AST tests under `tests/arch/`. They fail loudly when boundaries erode.

| Invariant | Tool |
|---|---|
| `gui/**` may not reference `ani-cli` outside `gui/src-tauri/src/anicli/path.rs` | ripgrep + allowlist |
| `ani-cli` must not contain `gui/`, `tauri`, `axum`, etc. | grep |
| Frontend imports no Rust types except generated `bindings/*.ts` | custom ESLint rule |
| Every `#[tauri::command]` returns `Result<T, AniError>` | syn-based audit |
| No hardcoded English in `.svelte` files (must go through `m.<key>()`) | regex test, allowlist for `aria-*`, `data-testid` |
| Crate dependency direction (`proxy` doesn't depend on `tauri`; `cache` doesn't depend on `reqwest`) | `cargo-deny` + `cargo-modules` |
| Forbidden APIs in bash: `awk`, GNU-only flags | grep |
| Tauri capability allowlist diffs require a `SECURITY.md` update | git diff hook |

## Mutation testing (deferred)

Trigger condition: after M0 + M1 are green for 30 days and CI duration stays under 8 minutes total.

- **Rust**: `cargo-mutants` nightly, scoped to `proxy/`, `cache/`, `history/`, `anicli/`. Target survival rate < 15%.
- **TS**: `stryker-js` nightly, scoped to `lib/` (DOM mutation noise on components is too high).
- **Bash**: no mature mutation tool exists. We compensate with property tests on pure functions and high-coverage acceptance tests.

## Test-discipline expectations for AI agents

Every PR shows the red→green pair in `git log`:

```sh
git log --oneline --grep '^test(red)' | head -20
```

Reconstructing the spec from these commits should be readable. Tests are documentation of intent; commit messages are documentation of motion.
