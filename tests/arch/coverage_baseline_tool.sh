#!/bin/sh
# Arch-layer wrapper around the node:test suite for
# tools/check-coverage-baseline.mjs. The script itself is plain
# Node, so its tests stay outside the Vitest/cargo trees — this
# shim just exposes them to the arch runner (and thus to arch.yml).

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$REPO_ROOT"
node --test tests/tools/check-coverage-baseline.test.mjs
