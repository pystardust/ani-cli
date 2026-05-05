#!/bin/sh
# Architectural invariant: the backend never returns localized strings.
# Errors are stable i18n keys (`error.scraper.timeout`) — the frontend
# resolves them.
#
# Concrete checks:
#   1. AniError variants in src/error.rs all have an entry in key().
#   2. Every constant in src/i18n.rs starts with "error." and has at
#      least three dot-separated segments.
#   3. (Future, when frontend lands) No hardcoded English text in
#      .svelte files except inside aria-* / data-testid attributes.
#
# Most of the actual checking lives inside the Rust unit tests in
# error.rs and i18n.rs (every_variant_has_a_stable_key,
# every_key_is_well_formed). This script is a fast grep-level
# canary so a contributor running `bash tests/arch/run-all.sh` gets a
# quick "yes, the i18n discipline is on" signal without needing the
# Rust toolchain.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -f gui/src-tauri/src/error.rs ]; then
    printf 'arch/i18n: error.rs not present yet — skipping\n'
    exit 0
fi

failed=0

# 1. error.rs must contain a `fn key(&self) -> &'static str` with arms
# for every variant. Variant names are pulled from the enum AniError
# definition; every name should appear at least twice (once in the
# enum, once in the key() match).
variants=$(awk '
    /^pub enum AniError/ { in_enum = 1; next }
    in_enum && /^}/ { in_enum = 0 }
    in_enum {
        # Match `Variant {` or `Variant,` or `Variant\n` after stripping
        # leading whitespace.
        gsub(/^[[:space:]]+/, "")
        if ($0 ~ /^[A-Z][A-Za-z0-9]*[[:space:]]*[\{,]/) {
            sub(/[\{,].*$/, "")
            gsub(/[[:space:]]/, "")
            if ($0 != "") print $0
        }
    }
' gui/src-tauri/src/error.rs)

for v in $variants; do
    # Must appear in key() match (Self::Variant ... => "...")
    if ! grep -q "Self::$v" gui/src-tauri/src/error.rs; then
        printf 'arch/i18n FAIL: AniError::%s has no arm in key()\n' "$v" >&2
        failed=1
    fi
done

# 2. i18n.rs constants must be well-formed.
if [ -f gui/src-tauri/src/i18n.rs ]; then
    while IFS= read -r line; do
        # Match e.g.: pub const NAME: &str = "error.scope.thing";
        value=$(printf '%s\n' "$line" | sed -nE 's/.*"([^"]*)".*/\1/p')
        [ -z "$value" ] && continue
        case "$value" in
            error.*.*) ;; # ok
            *)
                printf 'arch/i18n FAIL: i18n constant value %s does not match error.scope.name\n' "$value" >&2
                failed=1
                ;;
        esac
    done <<EOF
$(grep -E '^pub const [A-Z_]+: &str' gui/src-tauri/src/i18n.rs)
EOF
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/i18n PASS (variants checked: %s)\n' "$(printf '%s\n' "$variants" | wc -l | tr -d ' ')"
fi
exit "$failed"
