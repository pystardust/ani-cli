#!/usr/bin/env bats
#
# Unit tests for ani-cli's `dep_ch` (lines 122-127).
#
# Contract:
#   - Takes one or more dependency names as positional args.
#   - For each, runs `command -v` on the first whitespace-separated word.
#   - If absent, calls `die` (printf to stderr + exit 1).
#   - If all present, returns 0 with no output.

load '../helpers/loader'

@test "dep_ch: returns 0 when all deps are present" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch bash sh cat'
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}

@test "dep_ch: dies with exit 1 when a dep is missing" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch __definitely_not_a_real_cmd__'
    [ "$status" -eq 1 ]
    [[ "$output" =~ "not found" ]]
    [[ "$output" =~ "__definitely_not_a_real_cmd__" ]]
}

@test "dep_ch: only checks the first whitespace-separated word" {
    # `dep_ch "bash --some-flag"` should check `bash`, not the full string.
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch "bash --some-flag"'
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}

@test "dep_ch: stops at the first missing dep" {
    # First arg present, second missing; should fail on the second.
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch bash __also_missing__ cat'
    [ "$status" -eq 1 ]
    [[ "$output" =~ "__also_missing__" ]]
    # cat shouldn't be referenced in the error message
    [[ ! "$output" =~ "\"cat\"" ]]
}
