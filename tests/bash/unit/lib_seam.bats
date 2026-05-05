#!/usr/bin/env bats
#
# Tests the library-mode source seam.
#
# Sourcing ani-cli should let test files reuse its functions without invoking
# the main flow at the bottom of the script. The seam is a single line near
# the script bottom:
#
#     [ -n "$__ANI_CLI_LIB__" ] && return 0
#
# When __ANI_CLI_LIB__ is set, the script defines its functions and returns
# immediately. When unset, the script runs normally.
#
# This is the only patch carried on top of upstream ani-cli. The same patch is
# proposed upstream so the carried diff can drop to zero.
#
# Each test wraps the source in `timeout 3`: without the guard, sourcing
# reaches the script's interactive `read -r query` loop and hangs forever.
# With the guard, the script returns immediately. The timeout makes the
# red/green failure crisp instead of an indefinite hang.

load '../helpers/loader'

@test "sourcing ani-cli with __ANI_CLI_LIB__=1 returns within 3s with no stdout" {
    run timeout 3 bash -c '__ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null'
    assert_success
    [ -z "$output" ] || {
        printf 'unexpected stdout: %s\n' "$output" >&2
        return 1
    }
}

@test "sourcing ani-cli with __ANI_CLI_LIB__=1 does not exit the parent shell" {
    run timeout 3 bash -c '__ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null; echo OK'
    assert_success
    assert_output 'OK'
}

@test "sourcing ani-cli with __ANI_CLI_LIB__=1 makes pure functions callable" {
    # After sourcing in library mode, nth is callable. With empty stdin it
    # returns 1 per its own contract (line 21 of ani-cli).
    run timeout 3 bash -c '__ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null; printf "" | nth "select"'
    [ "$status" -eq 1 ]
}

@test "sourcing ani-cli with __ANI_CLI_LIB__=1 defines version_number" {
    # Confirms the seam is placed AFTER the variable declarations near the
    # top of the script — so the test toolchain has access to the same
    # constants ani-cli uses internally.
    run timeout 3 bash -c '__ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null; printf %s "$version_number"'
    assert_success
    [ -n "$output" ]
}
