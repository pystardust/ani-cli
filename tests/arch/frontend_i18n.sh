#!/bin/sh
# Architectural invariant: every visitor-facing string in routes and
# shared components is sourced from Paraglide.
#
# The check is intentionally conservative: it flags template text that
# looks like a sentence (starts with a capital letter, contains a space
# or sentence punctuation, ends before a closing tag) inside a .svelte
# file under src/routes/ or src/lib/components/. False positives can be
# whitelisted by including the substring `<!-- i18n-ignore -->` on the
# offending line.
#
# Catching every hardcoded literal is an open problem (template
# expressions, dynamic class names, accent-mark heuristics). This sniff
# is the cheap canary that catches the obvious "I forgot to wrap that
# label in m.foo()" mistakes; the contributor doc explains the
# convention in detail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ROUTES_DIR="$REPO_ROOT/gui/frontend/src/routes"
COMPONENTS_DIR="$REPO_ROOT/gui/frontend/src/lib/components"

if [ ! -d "$ROUTES_DIR" ]; then
    printf 'arch/frontend_i18n: routes/ not present yet — skipping\n'
    exit 0
fi

failed=0
# Build the literal `${` here so shellcheck doesn't flag it as an
# unexpanded variable inside a single-quoted grep pattern.
dollar_brace='$''{'
hits=$(mktemp)
trap 'rm -f "$hits"' EXIT

# Sentence-ish text between a closing `>` and an opening `<` —
# capitalised first word, includes a space, optionally ends with a
# period / question mark / exclamation. Excludes lines containing
# `m.` (Paraglide message call), `{` (template expression), or the
# explicit ignore comment.
find "$ROUTES_DIR" "$COMPONENTS_DIR" -name '*.svelte' 2>/dev/null | while IFS= read -r f; do
    grep -nE '>[[:space:]]*[A-Z][a-zA-Z]+[[:space:]]+[a-zA-Z]+[[:space:]a-zA-Z.,!?'"'"'-]*[<]' "$f" |
        grep -v 'i18n-ignore' |
        grep -v '\bm\.' |
        grep -vF "$dollar_brace" | # template expression
        grep -v '{[a-z]' || true
done >"$hits"

if [ -s "$hits" ]; then
    printf 'arch/frontend_i18n FAIL: hardcoded sentence-like text found:\n' >&2
    sed 's/^/  /' "$hits" >&2
    printf '\nWrap each through Paraglide (m.foo()), or annotate the line with <!-- i18n-ignore --> if it is genuinely not a translatable string.\n' >&2
    failed=1
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/frontend_i18n PASS\n'
fi
exit "$failed"
