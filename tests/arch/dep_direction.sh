#!/bin/sh
# Architectural invariant: layer dependencies in `gui/backend/` flow in
# one direction only, and the frontend never reaches into the backend
# (or anywhere else) by relative import.
#
# Concretely checked:
#
#   1. `gui/backend/src/cache/`     never `use`s reqwest. Caches read
#      from / write to disk + SQLite; HTTP fetches belong to the
#      `meta::*` and `proxy::*` layers.
#
#   2. `gui/backend/src/proxy/`     never `use`s `crate::meta`. The
#      streaming proxy is mechanical (tokenize, fetch upstream, rewrite
#      manifest); it doesn't touch metadata clients.
#
#   3. `gui/backend/src/anicli/`    never `use`s `crate::meta`,
#      `crate::commands`, or `crate::api`. The subprocess driver sits
#      below all three.
#
#   4. `gui/frontend/src/`           imports nothing from outside its
#      own `src/` tree (no `../../backend`, no `../../tools`, etc.).
#      The auto-generated bindings carve-out lives at
#      `src/lib/bindings/` and is the only allowed exception — but
#      it doesn't exist yet.
#
# Failures print the offending file:line so cause is obvious.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui/backend ]; then
    printf 'arch/dep_direction: gui/backend/ does not exist yet — skipping\n'
    exit 0
fi

failed=0

assert_no_match() {
    label="$1"
    pattern="$2"
    shift 2
    # `set -e` plus a successful grep returns 0; we want to fail when
    # the grep DOES find a match. Capture and check explicitly.
    if hits=$(grep -nE "$pattern" "$@" 2>/dev/null); then
        if [ -n "$hits" ]; then
            printf '\n[arch/dep_direction] %s\n' "$label"
            printf '%s\n' "$hits"
            failed=1
        fi
    fi
}

# 1. cache/ doesn't depend on reqwest — caches don't fetch.
if [ -d gui/backend/src/cache ]; then
    files=$(find gui/backend/src/cache -type f -name '*.rs')
    if [ -n "$files" ]; then
        # shellcheck disable=SC2086
        assert_no_match \
            'cache/ may not import reqwest (the cache reads/writes locally; HTTP belongs to meta/ and proxy/)' \
            '^use[[:space:]]+reqwest[:;]|^use[[:space:]]+::reqwest[:;]' \
            $files
    fi
fi

# 2. proxy/ doesn't depend on the metadata clients.
if [ -d gui/backend/src/proxy ]; then
    files=$(find gui/backend/src/proxy -type f -name '*.rs')
    if [ -n "$files" ]; then
        # shellcheck disable=SC2086
        assert_no_match \
            'proxy/ may not import crate::meta (the streaming proxy is upstream-agnostic)' \
            '^use[[:space:]]+crate::meta[:;]' \
            $files
    fi
fi

# 3. anicli/ sits below meta/, commands/, api/ — none of those should
#    appear in its imports.
if [ -d gui/backend/src/anicli ]; then
    files=$(find gui/backend/src/anicli -type f -name '*.rs')
    if [ -n "$files" ]; then
        # shellcheck disable=SC2086
        assert_no_match \
            'anicli/ may not import crate::meta, crate::commands, or crate::api (it sits below all three)' \
            '^use[[:space:]]+crate::(meta|commands|api)[:;]' \
            $files
    fi
fi

# 4. Frontend never reaches outside its own src/ via a relative import.
#    Anything resolved through SvelteKit's `$lib` alias stays inside
#    src/lib/, which is fine. We're looking for `from '../../...` chains
#    that escape src/, which would hit gui/backend or worse.
if [ -d gui/frontend/src ]; then
    files=$(find gui/frontend/src -type f \( -name '*.ts' -o -name '*.svelte' \))
    if [ -n "$files" ]; then
        # The `\.\.\/\.\.\/\.\.` pattern catches `../../..` chains that
        # escape src/<area>/<file>.ts → src/<area>/ → src/ → gui/frontend/.
        # shellcheck disable=SC2086
        assert_no_match \
            'frontend/src/ may not relative-import outside its own src/ tree (chain of `../../..`)' \
            "from[[:space:]]+['\"]\\.\\./\\.\\./\\.\\." \
            $files
    fi
fi

if [ "$failed" -ne 0 ]; then
    printf '\narch/dep_direction: failed — see violations above.\n'
    exit 1
fi

printf 'arch/dep_direction: ok\n'
