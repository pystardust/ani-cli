# AGENTS.md

Operational contract for any AI agent (Claude Code, Codex, others) working in this repository.

## 1. Project map

`ani-gui` is a fork of [`pystardust/ani-cli`](https://github.com/pystardust/ani-cli) that adds a desktop GUI on top of the existing CLI. The repo holds **two peer artifacts**:

- `ani-cli` (root) — the original 666-line POSIX-shell anime scraper, vendored from upstream and intentionally kept untouched.
- `gui/` — the desktop app. Electron shell (`gui/electron/`) hosts a SvelteKit static SPA (`gui/frontend/`) and launches a Rust sidecar (`gui/backend/`) that drives `ani-cli` strictly as a subprocess.

Read first:

- `docs/architecture.md` — components, data flow, why a local desktop app exists at all
- `docs/testing.md` — test pyramid and how to run each layer
- `docs/development.md` — dev environment, build, debug
- `docs/i18n.md` — adding a locale
- `docs/title-resolution.md` — the cross-API bridge (Kitsu ↔ allmanga ↔ MAL ↔ aniskip / AniList) and how disambiguation by episode count works
- `docs/proposals/` — future-feature proposals (Cast/multi-viewer, etc.)

## 2. Test discipline (non-negotiable, TDD)

Every change starts red:

1. Write or modify a test first. Commit it failing, with subject prefix `test(red): …`.
2. Make it pass with the minimum code. Commit with prefix `feat(green): …` or `fix(green): …`.
3. Refactor only after green. Commit with prefix `refactor: …` and prove tests still pass.

A PR with a `feat`/`fix` commit lacking a paired `test(red)` predecessor will be rejected. `git log --grep '^test(red)'` reconstructs the spec.

Per layer:

- Bash changes require bats-core coverage (unit, network-mocked, or acceptance as appropriate).
- Rust changes require `cargo test`, plus a `proptest` if the function under change is pure.
- Frontend changes require a `vitest` test (component or store) and an acceptance test if a user-visible flow changes.

Architectural invariants in `tests/arch/` are load-bearing. Do not weaken them — extend them.

Never modify a test to make production code pass. Modify production code, or change the test in its own `test(red)` commit with a written justification in the body.

The full pyramid (unit → acceptance → e2e → property → architectural invariants → mutation) lives in `docs/testing.md`.

**The CRAP ceilings (`crap.max_le`, `crap.p95_le`, `crap.high_risk_le` in `coverage-baseline.json`) are firm.** A PR that would push a file's CRAP above `max_le`, or push the count of high-risk files above `high_risk_le`, must refactor — split the file, extract helpers, cover more code — rather than raise the ceiling. The historical pattern of bumping the ceiling on every feature was lenient by accident; bringing code under a fixed bar is the actual quality signal. The percentage / kcov baselines (`rust.*`, `frontend.*`, `bash.*`) may still be refreshed via `node tools/check-coverage-baseline.mjs --update` when tests were deliberately added or removed — never as a workaround for new code that skipped testing.

**Svelte component logic must be testable.** The M3 design + UX detour shipped several pieces of behaviour inside `.svelte` files (BackButton depth tracking, topbar dropdown state machine, detail-page URL `$effect`s, hero rotation). Mounting Svelte 5 components against SvelteKit's runtime in vitest is brittle, so the rule is: **when you find yourself writing more than a couple of lines of imperative logic inside a `<script>`, extract it into a sibling `.ts` module under `$lib` and unit-test the module.** The component becomes a thin adapter that pulls inputs from the Svelte runtime and hands them to the pure function. `$lib/history/nav-depth.ts` is the canonical example — the layout's `afterNavigate` hook is now four lines of glue around a tested function.

Known test debt (extract + unit-test next time you touch them):

- Topbar live-results dropdown state machine in `+layout.svelte` (debounced search, ↑/↓ navigation, blur-dismiss timing, recent-search persistence).
- Detail-page URL `$effect`s in `routes/anime/[id]/+page.svelte` (`?page=` → `episodesPage`, `?ep=` → `highlightEp` + scrollIntoView, `consumedEp` guard against re-firing).
- Hero rotation timer in `routes/+page.svelte` (3-item cycle, pause on hover/focus, `prefers-reduced-motion` skip).

## 3. CLI script formatting parity (hard rule)

`ani-cli` (the root script) is vendored from upstream `pystardust/ani-cli`. Touching it requires:

- The change must be a behavior change we also intend to upstream — not a stylistic preference.
- Formatting must match upstream's settings byte-for-byte:
  - `shellcheck -s sh -o all -e 2250`
  - `shfmt -i 4 -ci -d`
- Never reformat the script. Never add lint rules to it.

Only one carried patch is permitted: the `__ANI_CLI_LIB__` source-guard line near the bottom of the file (so tests can `source` the script as a library). This guard is also being proposed upstream; if accepted, our carried patch becomes zero.

## 4. Layer boundaries

Mechanical rules enforced by `tests/arch/boundaries.sh` and `tests/arch/i18n.sh`:

- `gui/**` may invoke `ani-cli` only through `gui/backend/src/anicli/` (subprocess). No sourcing, no path references elsewhere.
- The frontend never fetches an upstream URL directly. All stream traffic flows through the local proxy at `http://127.0.0.1:<port>/s/<token>/...`.
- SQLite holds metadata only. Image bytes live on the filesystem under `$XDG_CACHE_HOME/ani-gui/images/`.
- The backend never returns localized strings. It returns stable error keys (`error.search.no_results`); the frontend resolves them via Paraglide.

## 5. Rust conventions (`gui/backend/`)

- Errors: `thiserror`-based `AniError` enum at the library boundary. `anyhow` allowed only inside command bodies.
- Subprocess: `tokio::process::Command` with `kill_on_drop(true)`, `TERM=dumb`, `NO_COLOR=1`.
- HTTP: `axum` for serving, `reqwest` (rustls) for outbound. Two clients: `meta_http` for Kitsu/AniList, `proxy_http` for stream upstream.
- Logging: `tracing` + `tracing-subscriber`. No `println!` in production code.
- Forbidden: `sqlx` (overkill for local SQLite), `actix-web`, `openssl-sys`, `*` version ranges in `Cargo.toml`.

## 6. Frontend conventions (`gui/frontend/`)

- Every user-visible string goes through Paraglide. The `no-hardcoded-strings` ESLint rule enforces this; it allowlists only `aria-*`, `data-testid`, and dev-only strings.
- Use logical CSS properties (`margin-inline-start`, not `margin-left`) so adding RTL locales later is translation-only.
- No DOM-snapshot tests. Assert behavior: rendered text via `i18n.m`, role queries, user events.
- hls.js is used as a singleton inside `Player.svelte`; no `new Hls()` outside that component.

## 7. Design direction guard rails

UI is top priority for this project; defaults to Netflix-style polish. Avoid:

- Generic Tailwind/shadcn dark mode
- Glassmorphism without purpose
- Neon-purple gradients
- AI-styled abstract blob backgrounds
- Auto-rotating carousels (carousels respond to user scroll, not timers)
- Inter-everywhere typography

Embrace:

- Dynamic per-anime theming using AniList's `coverImage.color` for accents on detail/watch pages
- Editorial typography pairing (display face + body face) — pick concretely at the start of M3
- Motion as structure, not decoration: elastic-eased carousels, parallax cards, shared-element page transitions, theater-dim into playback
- Subtle anime motifs: oversized tabular numerals for episode counts, manga-page-inspired dividers used sparingly. No literal sakura or holographic katakana banners.
- Player chrome that auto-hides cleanly (Apple TV+ feel, not VLC)

## 8. `frontend-design` skill usage

When invoking the Anthropic `frontend-design:frontend-design` skill for component generation:

- Always pass design-direction constraints (§7) in the prompt verbatim
- Always run a sub-agent reviewer pass against the output before merging
- Never accept generated code as-is — the skill has produced repetitive AI-styled output before

## 9. UI is top priority

Once a milestone affects the UI surface, treat the work as v1-quality, not as a quick patch. Specifically: M4 ani-skip integration explicitly triggers a UI revisit, since the player overlay changes shape when intro-skip exists.

## 10. PR conventions

Set the assignee on every PR (`gh pr create --assignee @me`). Add a label from the repo's existing set (`bug`, `enhancement`, `documentation`, …) when one obviously fits the change; skip the label otherwise — don't invent new ones without asking.

## 11. System-modifying actions require explicit approval

This rule holds in **all modes**, including auto mode. Pause and surface a request — never silently execute — for any action that:

- Requires `sudo` or any privilege escalation
- Modifies anything under `/etc`, `/usr`, `/opt`, `/var`, system services, or systemd units
- Modifies user-global state outside the repo: `~/.bashrc`, `~/.zshrc`, `~/.profile`, `~/.cargo/`, `~/.rustup/`, `~/.nvm/`, `~/.npm/`, `~/.local/bin/`, `~/.config/` (other than this app's own config), the user's `PATH`, `corepack enable`, `cargo install -g`, etc.
- Installs system packages (`apt`, `dnf`, `pacman`, `brew`, `pkg`, `snap`, `flatpak install`)
- Modifies firewall rules, network config, environment variables persisted to shell rc files
- Affects any file outside the repo working directory tree

Project-local writes inside the repo are always fine (the test toolchain installer at `tests/bash/helpers/install-bats.sh` writes only to `tests/bash/.bats-vendor/`, which is gitignored — that's project-local and proceeds).

When pausing for approval, explain what the action does, why it's needed, and what the alternative is if the user declines. Examples of safe alternatives: ship a Docker dev image rather than asking the user to install system packages; vendor a tool into the repo rather than `cargo install -g`; document the requirement in `docs/development.md` so the user installs it themselves.

## 12. Git hygiene

- **Stage files individually, by full path.** Never use `git add .`, `git add -A`, `git add -u`, or directory-level adds like `git add docs/`. Each file goes into the index by name. This forces an intentional review of every file in every commit and prevents accidental inclusion of secrets, scratch files, or unrelated edits.
- **Commit subjects use the conventional prefix matching the change kind**: `test(red): …`, `feat(green): …`, `fix(green): …`, `refactor: …`, `chore: …`, `docs: …`, `chore(deps): …`, `chore(ci): …`. Anything that introduces a `feat` or `fix` must have a paired predecessor `test(red): …` commit (see §2).
- **No `git push --force` to `master`.** If a force-push is genuinely needed (e.g. accidentally committed credentials), pause and confirm with the user before doing it.
- **Never `--no-verify` past hooks** unless the user explicitly directs it. If a pre-commit hook fails, fix the underlying issue.

## 13. Pointers

- `docs/architecture.md` — public architecture
- `docs/testing.md` — test pyramid, fixture management, coverage targets
- `docs/development.md` — dev setup
- `docs/i18n.md` — locale-addition guide
- `docs/proposals/cast-multiviewer.md` — Cast/multi-viewer future-feature proposal
