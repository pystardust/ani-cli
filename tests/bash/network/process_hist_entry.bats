#!/usr/bin/env bats
#
# Tests for ani-cli's `process_hist_entry` (lines 346-352).
#
# Contract:
#   - Reads $id, $title, $ep_no from caller (one history line).
#   - Calls episodes_list "$id" to get the show's full list.
#   - Takes the episode AFTER the current ep_no.
#   - Updates the "(N episodes)" suffix in title to the latest ep count.
#   - If a next episode exists, prints "id\ttitle - episode N".
#   - If at the last episode, prints nothing (no next episode line).
#
# We mock episodes_list as a function override so this test stays hermetic.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # Mock episodes_list to return a known episode set.
    episodes_list() {
        printf '1\n2\n3\n4\n5\n'
    }
    export -f episodes_list
}

@test "process_hist_entry: emits id\\tnew-title\\tnext-ep when more episodes are available" {
    id='abc123'
    title='Test Anime (3 episodes)'
    ep_no='2'
    output=$(process_hist_entry)
    # Title's "3 episodes" updates to "5 episodes" (latest from mocked list).
    # Next episode after 2 is 3.
    expected="abc123"$'\t'"Test Anime (5 episodes) - episode 3"
    [ "$output" = "$expected" ]
}

@test "process_hist_entry: emits nothing when caller is at the last episode" {
    id='abc123'
    title='Test Anime (5 episodes)'
    ep_no='5'
    output=$(process_hist_entry)
    [ -z "$output" ]
}

@test "process_hist_entry: refreshes title's episode count from the mocked list" {
    id='abc123'
    title='Test Anime (3 episodes)'   # caller's stale view: 3 episodes
    ep_no='1'
    output=$(process_hist_entry)
    # The list has 5 episodes, so the title gets updated.
    [[ "$output" == *"(5 episodes)"* ]]
    # And the next episode after 1 is 2.
    [[ "$output" == *"- episode 2" ]]
}
