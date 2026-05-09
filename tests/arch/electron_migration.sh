#!/bin/sh
# Architectural invariant: the Tauri → Electron migration must stay
# done. Historical comments referencing Tauri ("this used to be a
# tauri::command") are fine — they explain the migration. But active
# code references (use tauri::, #[tauri::command], src-tauri/
# resource paths, tauri-build / tauri-cli in Cargo.toml,
# @tauri-apps/* in package.json) would mean a real regression: a
# dependency or import sneaking back in.
#
# The check intentionally targets ACTIVE references, not
# documentation. Greps look at code lines (rust use, attributes,
# directory references in JSON) rather than freeform text.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui ]; then
    printf 'arch/electron_migration: gui/ does not exist yet — skipping\n'
    exit 0
fi

failed=0

GREP_EXCLUDE='--exclude-dir=target --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --exclude-dir=.svelte-kit'

# 1. No active `use tauri::` or `extern crate tauri` in Rust.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE \
    '^\s*(use|extern crate)\s+tauri([_a-zA-Z]*)?(::|;)' \
    gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/electron_migration FAIL: active Tauri import in gui/ Rust:\n%s\n' "$matches" >&2
    failed=1
fi

# 2. No `#[tauri::command]` (or any tauri::-prefixed attribute) on
#    function definitions. Only flag the attribute when it's on a
#    real attribute line (whitespace + #[tauri::); doc comments and
#    string literals are allowed to mention the historical shape.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '^\s*#\[tauri::' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/electron_migration FAIL: tauri:: attribute in gui/:\n%s\n' "$matches" >&2
    failed=1
fi

# 3. Cargo.toml must not depend on any `tauri*` crate. The
#    migration removed tauri-build, tauri-cli, and tauri itself.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE \
    '^\s*tauri[a-zA-Z_-]*\s*=' \
    gui/backend/Cargo.toml 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/electron_migration FAIL: Tauri dep in Cargo.toml:\n%s\n' "$matches" >&2
    failed=1
fi

# 4. package.json must not depend on @tauri-apps/* anywhere under
#    gui/ (frontend or electron).
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE \
    '"@tauri-apps/' \
    gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/electron_migration FAIL: @tauri-apps dep in package.json:\n%s\n' "$matches" >&2
    failed=1
fi

# 5. No `src-tauri/` directory references in production paths
#    (config files, scripts, resource lookups). Comments mentioning
#    the historical name are allowed; this rule looks specifically
#    for paths used by tooling.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE \
    '"path":\s*"[^"]*src-tauri|"resourcePath":\s*"[^"]*src-tauri|^\s*"src-tauri/' \
    gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/electron_migration FAIL: src-tauri/ path reference in gui/ config:\n%s\n' "$matches" >&2
    failed=1
fi

# 6. The src-tauri/ directory itself must not exist — its presence
#    would mean someone reintroduced the Tauri scaffolding.
if [ -d gui/src-tauri ]; then
    printf 'arch/electron_migration FAIL: gui/src-tauri/ directory exists; Tauri scaffolding has returned.\n' >&2
    failed=1
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/electron_migration PASS\n'
fi
exit "$failed"
