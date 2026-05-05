#!/bin/sh
# Run every architectural-invariant check under tests/arch/. Each check is its
# own .sh file; this script aggregates and reports overall pass/fail. Exits
# non-zero on any failure.
#
# Stays cheap — these are grep/AST tests, not behavioral tests. The whole
# suite runs in seconds.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

overall=0
for check in "$SCRIPT_DIR"/*.sh; do
    name="$(basename "$check" .sh)"
    [ "$name" = 'run-all' ] && continue
    [ -f "$check" ] || continue
    printf '\n=== arch: %s ===\n' "$name"
    if ! sh "$check"; then
        overall=1
        printf '  FAIL: %s\n' "$name"
    else
        printf '  PASS\n'
    fi
done

exit "$overall"
