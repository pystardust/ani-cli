#!/bin/sh
# Run the entire bash test suite (unit + network + subprocess + acceptance +
# property) using the vendored bats. CI invokes this. Locally, run it after
# install-bats.sh.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../.bats-vendor"
BATS_BIN="$VENDOR_DIR/bats-core/bin/bats"
TESTS_BASH_DIR="$SCRIPT_DIR/.."

if [ ! -x "$BATS_BIN" ]; then
    printf 'bats not installed; run %s first\n' "$SCRIPT_DIR/install-bats.sh" >&2
    exit 1
fi

# Run each suite separately so failures are easier to locate. `bats` exits
# non-zero on the first failing suite; we keep going so the developer sees
# every failure at once, then exit non-zero at the end if any suite failed.
overall=0
for suite in unit network subprocess acceptance property; do
    dir="$TESTS_BASH_DIR/$suite"
    if [ ! -d "$dir" ]; then
        continue
    fi
    files=$(find "$dir" -type f -name '*.bats' | sort)
    if [ -z "$files" ]; then
        printf '  (no bats files in %s yet, skipping)\n' "$suite"
        continue
    fi
    printf '\n=== suite: %s ===\n' "$suite"
    if ! "$BATS_BIN" $files; then
        overall=1
    fi
done

exit "$overall"
