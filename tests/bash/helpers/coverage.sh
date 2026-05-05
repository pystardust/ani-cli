#!/bin/sh
# Produce coverage for the bash tests using kcov. Honors two thresholds set
# via env: COVERAGE_FAIL_UNDER_PURE (default 95) and COVERAGE_FAIL_UNDER_NETWORK
# (default 70). Exits non-zero if any threshold is missed.
#
# Layout produced:
#   coverage/bash/
#     pure/      ← coverage from tests/bash/unit/ + tests/bash/property/
#     network/   ← coverage from tests/bash/network/ + tests/bash/subprocess/
#     accept/    ← coverage from tests/bash/acceptance/
#     summary.txt

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../.bats-vendor"
BATS_BIN="$VENDOR_DIR/bats-core/bin/bats"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
COV_ROOT="$REPO_ROOT/coverage/bash"
THRESHOLD_PURE="${COVERAGE_FAIL_UNDER_PURE:-95}"
THRESHOLD_NETWORK="${COVERAGE_FAIL_UNDER_NETWORK:-70}"

if ! command -v kcov >/dev/null 2>&1; then
    printf 'kcov not installed; install with `apt-get install kcov` or skip with COVERAGE=0\n' >&2
    [ "${COVERAGE:-1}" = '0' ] && exit 0 || exit 1
fi

mkdir -p "$COV_ROOT/pure" "$COV_ROOT/network" "$COV_ROOT/accept"

run_kcov_for_suites() {
    out_dir="$1"
    shift
    for suite in "$@"; do
        dir="$REPO_ROOT/tests/bash/$suite"
        if [ ! -d "$dir" ]; then
            continue
        fi
        files=$(find "$dir" -type f -name '*.bats' | sort)
        [ -z "$files" ] && continue
        kcov --include-path="$REPO_ROOT/ani-cli" \
            --bash-dont-parse-binary-dir \
            "$out_dir" "$BATS_BIN" $files >/dev/null
    done
}

run_kcov_for_suites "$COV_ROOT/pure"    unit property
run_kcov_for_suites "$COV_ROOT/network" network subprocess
run_kcov_for_suites "$COV_ROOT/accept"  acceptance

extract_pct() {
    cov_json="$1/index.js"
    [ -f "$cov_json" ] || { printf '0'; return; }
    sed -nE 's/.*"covered_percent":([0-9.]+).*/\1/p' "$cov_json" | head -n1
}

pure_pct=$(extract_pct "$COV_ROOT/pure")
network_pct=$(extract_pct "$COV_ROOT/network")
accept_pct=$(extract_pct "$COV_ROOT/accept")

{
    printf 'bash coverage summary\n'
    printf '  pure          %s%% (threshold %s%%)\n' "$pure_pct" "$THRESHOLD_PURE"
    printf '  network/subp  %s%% (threshold %s%%)\n' "$network_pct" "$THRESHOLD_NETWORK"
    printf '  acceptance    %s%%\n' "$accept_pct"
} | tee "$COV_ROOT/summary.txt"

below() {
    awk -v a="$1" -v b="$2" 'BEGIN{ exit !(a+0 < b+0) }'
}

failed=0
if below "$pure_pct" "$THRESHOLD_PURE"; then
    printf 'FAIL: pure coverage %s%% below threshold %s%%\n' "$pure_pct" "$THRESHOLD_PURE" >&2
    failed=1
fi
if below "$network_pct" "$THRESHOLD_NETWORK"; then
    printf 'FAIL: network/subprocess coverage %s%% below threshold %s%%\n' "$network_pct" "$THRESHOLD_NETWORK" >&2
    failed=1
fi
exit "$failed"
