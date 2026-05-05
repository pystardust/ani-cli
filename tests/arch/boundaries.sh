#!/bin/sh
# Architectural invariant: gui/ may invoke ani-cli only via subprocess —
# never via sourcing, never by importing function bodies.
#
# Concretely:
#   1. The string "source ./ani-cli", ". ./ani-cli", or "source ../../ani-cli"
#      must NOT appear under gui/ (the test seam __ANI_CLI_LIB__ is for
#      tests/bash/ only).
#   2. The string "ani-cli" inside gui/ must only appear in known
#      whitelisted contexts: comments, docstrings, the literal binary
#      name passed to Command::new, or the resource path in
#      tauri.conf.json. Anything else is a layer violation.
#
# This invariant exists because vendoring ani-cli straight from upstream
# is the only sustainable strategy; the moment gui/ source-imports the
# script, every upstream rebase becomes a merge minefield.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui ]; then
    printf 'arch/boundaries: gui/ does not exist yet — skipping\n'
    exit 0
fi

failed=0

# Build artifacts (target/, node_modules/, bundles) are gitignored but
# may sit on disk and contain copies of the script. Skip them.
GREP_EXCLUDE='--exclude-dir=target --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --exclude-dir=.svelte-kit'

# 1. No sourcing.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '(^|[[:space:]])((source|\.)[[:space:]]+["'"'"']?[^"'"'"' ]*ani-cli)' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/boundaries FAIL: gui/ sources ani-cli (forbidden):\n%s\n' "$matches" >&2
    failed=1
fi

# 2. __ANI_CLI_LIB__ guard outside tests/bash/.
# shellcheck disable=SC2086
matches=$(grep -rn $GREP_EXCLUDE '__ANI_CLI_LIB__' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/boundaries FAIL: __ANI_CLI_LIB__ appears in gui/ (test seam only):\n%s\n' "$matches" >&2
    failed=1
fi

# 3. ani-cli's internal globals (allanime_*, search_anime, etc.) must
# never appear in gui/. If a Rust file references them by name, it's
# almost certainly a re-implementation of scraping logic that should
# instead be a subprocess call.
banned_symbols='search_anime\|episodes_list\|get_episode_url\|decode_tobeparsed\|allanime_key\|allanime_base\|generate_link\|provider_init'
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE "($banned_symbols)" gui/ 2>/dev/null \
    | grep -v 'tests/.*proxy_router.rs' \
    | grep -v 'docs\.rs\|//[!/].*' \
    || true)
# Filter doc-comment matches (allow them — they reference the ani-cli
# function the GUI driver hits, which is informational).
filtered=$(printf '%s\n' "$matches" | awk -F: '
    # Skip empty lines
    NF == 0 { next }
    # Skip lines whose remainder is a doc comment
    {
        rest = ""
        for (i = 3; i <= NF; i++) rest = rest ":" $i
        gsub(/^:/, "", rest)
        if (rest ~ /^[[:space:]]*\/\/[!\/]/) next
        print
    }
')
if [ -n "$filtered" ]; then
    printf 'arch/boundaries FAIL: gui/ references ani-cli internal globals (suspect re-implementation):\n%s\n' "$filtered" >&2
    failed=1
fi

# 4. Reverse: ani-cli the script must not contain GUI references.
if [ -f ani-cli ]; then
    matches=$(grep -nE '(gui/|tauri::|use ani_gui|svelte)' ani-cli || true)
    if [ -n "$matches" ]; then
        printf 'arch/boundaries FAIL: ani-cli script contains GUI references:\n%s\n' "$matches" >&2
        failed=1
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/boundaries PASS\n'
fi
exit "$failed"
