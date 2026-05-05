#!/usr/bin/env bats
#
# Tests for ani-cli's `episodes_list` (lines 336-342) with mocked curl.
#
# Contract:
#   - $1 = show id.
#   - Calls curl POST to ${allanime_api}/api with a GraphQL "show" query.
#   - Parses availableEpisodesDetail.${mode} into one episode number per
#     line, sorted ascending numerically.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/curl_mock'
    export CURL_MOCK_LOG="$BATS_TEST_TMPDIR/curl.log"
    : >"$CURL_MOCK_LOG"
    mode='sub'
}

@test "episodes_list: produces 25 ascending episode numbers from canned response" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/episodes_attack_on_titan.json"
    output=$(episodes_list "MWMqGjvE2yBb2zoiv")
    line_count=$(printf '%s\n' "$output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 25 ]
    first_line=$(printf '%s\n' "$output" | head -n1)
    last_line=$(printf '%s\n' "$output" | tail -n1)
    [ "$first_line" = "1" ]
    [ "$last_line" = "25" ]
}

@test "episodes_list: handles fractional episodes (1.5)" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/episodes_short.json"
    output=$(episodes_list "shortid123")
    # sort -n -k 1 with default sort: 1, 1.5, 2, 3
    expected=$(printf '1\n1.5\n2\n3')
    [ "$output" = "$expected" ]
}

@test "episodes_list: dub mode with empty dub list yields no output" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/episodes_short.json"
    mode='dub'
    output=$(episodes_list "shortid123")
    [ -z "$output" ]
}

@test "episodes_list: passes the show id in the POST body" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/episodes_attack_on_titan.json"
    episodes_list "MWMqGjvE2yBb2zoiv" >/dev/null
    log=$(cat "$CURL_MOCK_LOG")
    [[ "$log" == *"MWMqGjvE2yBb2zoiv"* ]]
    [[ "$log" == *"-X POST"* ]]
    [[ "$log" == *"https://api.allanime.day/api"* ]]
}
