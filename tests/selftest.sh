#!/bin/sh
set -eu

repo_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_dir"

fail() {
  printf "FAIL: %s\n" "$*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "missing dependency: $1"
}

need rg

tmp="${TMPDIR:-/tmp}/ani-cli-selftest.$$"
trap 'rm -rf "$tmp"' EXIT INT TERM
mkdir -p "$tmp"

export ANI_CLI_HIST_DIR="$tmp/state"

sh -n ./ani-cli || fail "sh -n failed"

./ani-cli -h | rg -n -- '--seen' >/dev/null || fail "help missing --seen"
./ani-cli -h | rg -n -- '--history-on-exit' >/dev/null || fail "help missing --history-on-exit"

./ani-cli --delete >/dev/null 2>&1 || fail "--delete failed"
[ -f "$ANI_CLI_HIST_DIR/ani-hsts" ] || fail "histfile missing after --delete"
[ -f "$ANI_CLI_HIST_DIR/ani-seen" ] || fail "seenfile missing after --delete"
[ ! -s "$ANI_CLI_HIST_DIR/ani-hsts" ] || fail "histfile not empty after --delete"
[ ! -s "$ANI_CLI_HIST_DIR/ani-seen" ] || fail "seenfile not empty after --delete"

set +e
./ani-cli --seen >/dev/null 2>&1
rc=$?
set -e
[ "$rc" -ne 0 ] || fail "--seen should fail when history is empty"

printf "OK\n"

