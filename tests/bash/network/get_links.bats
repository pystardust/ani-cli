#!/usr/bin/env bats
#
# Tests for ani-cli's `get_links` (lines 142-169) with mocked curl.
#
# Contract:
#   - $* = path/query appended to https://${allanime_base}.
#   - Calls curl GET to that URL with the allanime referer.
#   - Parses the response JSON, extracts the embedded "link" + "resolutionStr".
#   - Branches by URL marker:
#       repackager.wixmp.com → multi-quality emit (one line per detected quality)
#       master.m3u8          → HLS link with cc> marker, subtitle line, refr line
#       *                    → emit the parsed link as-is
#   - Additionally: if $* contains tools.fast4speed.rsvp, emit "Yt >$*".
#   - Always prints a colored "<provider_name> Links Fetched" to stderr.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/curl_mock'
    # cache_dir is used by the m3u8 branch to write suburl + m3u8_refr files.
    # Some tests rely on that directory existing.
    cache_dir="$BATS_TEST_TMPDIR"
    provider_name='wixmp'
}

@test "get_links: plain mp4 response emits a single quality > URL line" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/embed_plain.json"
    output=$(get_links "/some/path" 2>/dev/null)
    [ "$output" = "720 >https://sharepoint.example/video.mp4" ]
}

@test "get_links: tools.fast4speed.rsvp path emits Yt > <path>" {
    # The Yt branch is triggered by the path itself, regardless of response.
    # An empty response is fine since the function still emits the Yt line.
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/embed_plain.json"
    yt_path="/anime/clock?id=tools.fast4speed.rsvp"
    output=$(get_links "$yt_path" 2>/dev/null)
    [[ "$output" == *"Yt >$yt_path"* ]]
}

@test "get_links: wixmp response expands quality variants" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/embed_wixmp.json"
    output=$(get_links "/some/path" 2>/dev/null)
    # The wixmp branch parses the URL ".../,1080,720,480,/mp4/..." and emits
    # one line per quality. We assert the quality numbers appear, not the
    # exact URL form (the rewrite is regex-driven and brittle to test
    # byte-for-byte).
    [[ "$output" == *"1080 >"* ]]
    [[ "$output" == *"720 >"* ]]
    [[ "$output" == *"480 >"* ]]
}

@test "get_links: m3u8 response emits a cc>-marked link and a subtitle line" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/embed_hls.json"
    # The m3u8 branch fetches the master.m3u8 again; mock that secondary
    # call too. We have the dispatch helper pick a different fixture based on
    # whether the URL contains "master.m3u8".
    dispatch_for_get_links() {
        local args="$*"
        if [[ "$args" == *"master.m3u8"* ]]; then
            # Second curl call: the actual m3u8 master playlist. We return a
            # tiny minimal master playlist with one variant so the script's
            # parsing finds at least one bandwidth+resolution line.
            printf '%s' "$FIXTURES_DIR/allanime/master_min.m3u8"
        else
            printf '%s' "$FIXTURES_DIR/allanime/embed_hls.json"
        fi
    }
    export -f dispatch_for_get_links
    export CURL_MOCK_DISPATCH=dispatch_for_get_links
    unset CURL_MOCK_RESPONSE

    # Create the minimal master m3u8 fixture inline (very small; not worth
    # a separate test artifact).
    cat >"$FIXTURES_DIR/allanime/master_min.m3u8" <<'EOF'
#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1920x1080
1080/index.m3u8
EOF

    output=$(get_links "/some/path" 2>/dev/null)
    # Subtitle file should have been written.
    [ -f "$cache_dir/suburl" ]
    grep -q 'subtitle >https://hianime.example/sub.vtt' "$cache_dir/suburl"
    # The output contains a line with the cc> marker.
    [[ "$output" == *"cc>"* ]]

    rm -f "$FIXTURES_DIR/allanime/master_min.m3u8"
}

@test "get_links: prints '<provider> Links Fetched' to stderr" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/embed_plain.json"
    provider_name='hianime'
    stderr=$(get_links "/some/path" 2>&1 >/dev/null)
    [[ "$stderr" == *"hianime"* ]]
    [[ "$stderr" == *"Links Fetched"* ]]
}
