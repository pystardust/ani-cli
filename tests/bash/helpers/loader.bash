# Common loader for every .bats file in tests/bash/.

# Require bats >= 1.5.0 so flags on `run` (e.g. --separate-stderr, -N) work.
bats_require_minimum_version 1.5.0

#
# Usage at the top of a .bats file:
#
#   load '../helpers/loader'
#
# This file is sourced (not executed) and:
#   1. Resolves repo paths into shell variables (REPO_ROOT, ANI_CLI_PATH, FIXTURES_DIR)
#   2. Loads bats-support, bats-assert, and bats-file from the vendored toolchain
#   3. Provides a single helper, source_ani_cli_lib, that sources ani-cli with
#      the __ANI_CLI_LIB__ guard set so the script does not auto-execute.
#
# It does NOT source ani-cli on its own — tests opt in. Acceptance tests that
# run the real CLI never source it.

# shellcheck shell=bash

# Resolve paths. BATS_TEST_DIRNAME points at the directory of the .bats file
# being run; we walk up until we find the repo root marker.
__loader_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$__loader_dir/../../.." && pwd)"
ANI_CLI_PATH="$REPO_ROOT/ani-cli"
FIXTURES_DIR="$REPO_ROOT/tests/fixtures"
BATS_VENDOR="$REPO_ROOT/tests/bash/.bats-vendor"

export REPO_ROOT ANI_CLI_PATH FIXTURES_DIR BATS_VENDOR

# Load bats helper libraries from the vendored checkouts.
# bats-support must be loaded before bats-assert.
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-support/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-assert/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-file/load"

# source_ani_cli_lib — source the CLI with the library-mode guard set so the
# main dispatcher at the bottom of the script does not run. After this
# returns, every function defined in ani-cli is callable directly from the
# test.
#
# Usage:
#   source_ani_cli_lib
#   run nth "select" <<<"$some_input"
#
# Bats runs tests with `set -e` (errexit). ani-cli has lines in its setup
# block such as `[ -t 0 ] || (command -v dmenu && use_external_menu=2)` that
# return non-zero on machines without dmenu/rofi installed — innocuous in
# normal CLI execution but fatal under errexit. We briefly disable errexit
# around the source and restore it afterward.
source_ani_cli_lib() {
    if [ ! -r "$ANI_CLI_PATH" ]; then
        printf 'ANI_CLI_PATH not readable: %s\n' "$ANI_CLI_PATH" >&2
        return 1
    fi
    # Disable the bats ERR trap and errexit/errtrace for the rest of the test.
    #
    # bats v1.11 installs an ERR trap that fails the test on ANY non-zero
    # exit — including intentional ones that are part of ani-cli's contract:
    #   - `grep -m 1` finding nothing inside select_quality's case branch
    #   - `nth` returning 1 on empty stdin
    #   - the dmenu/rofi `command -v` checks in ani-cli's setup block
    #
    # ani-cli is a POSIX-sh script and never enables set -e itself; tests
    # should mirror that environment. Tests that need to assert on a
    # non-zero exit use `run` or explicit `$?` capture; the trap-driven
    # auto-fail does not match how the script is actually written.
    trap - ERR
    set +eE
    # shellcheck source=/dev/null
    __ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null || true
    return 0
}
