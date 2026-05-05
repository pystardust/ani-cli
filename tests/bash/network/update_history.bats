#!/usr/bin/env bats
#
# Tests for ani-cli's `update_history` (lines 354-362).
#
# Contract:
#   - If $id is found in $histfile, update that line's ep_no.
#   - Else, append a new line "ep_no\tid\ttitle".
#   - Writes atomically via $histfile.new + mv.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    histfile="$BATS_TEST_TMPDIR/ani-hsts"
    : >"$histfile"
}

@test "update_history: appends a new entry when id is absent" {
    cp "$FIXTURES_DIR/history/single.tsv" "$histfile"
    id='newid'
    ep_no='1'
    title='New Anime (12 episodes)'
    update_history
    # Two lines now: original + the new one.
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 2 ]
    grep -E "^1"$'\t'"newid"$'\t'"New Anime \(12 episodes\)$" "$histfile" >/dev/null
    # Original line untouched.
    grep -E "^5"$'\t'"abc123"$'\t'"Attack on Titan \(25 episodes\)$" "$histfile" >/dev/null
}

@test "update_history: updates ep_no on the matching id line" {
    cp "$FIXTURES_DIR/history/multi.tsv" "$histfile"
    id='def456'
    ep_no='4'
    title='Demon Slayer (26 episodes)'
    update_history
    # Line for def456 should have ep_no = 4 now (was 3).
    grep -E "^4"$'\t'"def456"$'\t'"Demon Slayer \(26 episodes\)$" "$histfile" >/dev/null
    # Other lines untouched.
    grep -E "^12"$'\t'"abc123"$'\t'"Attack on Titan \(25 episodes\)$" "$histfile" >/dev/null
    grep -E "^1"$'\t'"ghi789"$'\t'"Spy x Family \(12 episodes\)$" "$histfile" >/dev/null
    # Total line count unchanged.
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 3 ]
}

@test "update_history: appending to empty histfile creates one entry" {
    : >"$histfile"
    id='abc123'
    ep_no='1'
    title='Test (10 episodes)'
    update_history
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 1 ]
    grep -E "^1"$'\t'"abc123"$'\t'"Test \(10 episodes\)$" "$histfile" >/dev/null
}

@test "update_history: leaves no .new sidecar after the atomic move" {
    cp "$FIXTURES_DIR/history/single.tsv" "$histfile"
    id='abc123'
    ep_no='6'
    title='Attack on Titan (25 episodes)'
    update_history
    [ ! -f "${histfile}.new" ]
    [ -f "$histfile" ]
}
