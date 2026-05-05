#!/usr/bin/env bats
#
# Unit tests for ani-cli's `nth` (lines 19-36).
#
# Contract:
#   - Reads all of stdin into a buffer.
#   - Empty stdin → returns 1 with no output.
#   - Single-line stdin → outputs `cut -f2,3` of that line, returns 0.
#   - Multi-line stdin → invokes `launcher` to pick a line; tested with a
#     mocked launcher.
#
# `nth`'s contract for multi-line involves calling `launcher`, which in
# production goes through fzf/rofi/dmenu. We mock launcher inline in each
# multi-line test.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
}

@test "nth: empty stdin returns 1 with no output" {
    # nth is defined in our shell via setup. We capture its exit through $?
    # immediately (the loader has disabled the bats ERR trap so the
    # intentional return 1 doesn't fail the test).
    output=$(printf "" | nth "select" 2>&1)
    status=$?
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "nth: single-line stdin outputs cut -f2,3 of the line" {
    # Tab-separated: id1<TAB>title1<TAB>extra1
    output=$(printf 'id1\ttitle1\textra1\n' | nth "select")
    [ "$?" -eq 0 ]
    [ "$output" = $'title1\textra1' ]
}

@test "nth: single-line stdin with only two fields outputs field 2 alone" {
    # `cut -f2,3` on "id\ttitle" returns "title" (field 3 doesn't exist; cut omits it).
    output=$(printf 'id\ttitle\n' | nth "select")
    [ "$?" -eq 0 ]
    [ "$output" = "title" ]
}

@test "nth: multi-line stdin with mocked launcher picks the chosen line" {
    # Mock launcher: always pick the second line (returns its first field after the cut/tr pipeline).
    launcher() { sed -n '2p' | cut -d' ' -f1; }
    output=$(printf 'id1\ttitle1\textra1\nid2\ttitle2\textra2\nid3\ttitle3\textra3\n' | nth "select")
    [ "$?" -eq 0 ]
    [ "$output" = $'title2\textra2' ]
}

@test "nth: multi-line with launcher returning empty exits 1" {
    # Mock launcher returning nothing → nth's `[ -n "$line" ] || exit 1` branch.
    launcher() { :; }
    run --separate-stderr bash -c '
        __ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null
        launcher() { :; }
        printf "id1\ttitle1\textra1\nid2\ttitle2\textra2\n" | nth "select"
    '
    [ "$status" -eq 1 ]
}
