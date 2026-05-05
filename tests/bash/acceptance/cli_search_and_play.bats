#!/usr/bin/env bats
#
# Acceptance test: end-to-end "search → episode → play" pipeline.
#
# Runs the real ./ani-cli with ANI_CLI_PLAYER=debug, with curl shimmed on
# PATH and with a pre-built encrypted episode blob fixture. The script
# should reach the debug branch in play_episode and print the resolved
# stream URL to stdout.
#
# This is the only acceptance test that exercises the full pipeline:
# search_anime + episodes_list + get_episode_url (decode_tobeparsed +
# 5 parallel provider fetches + select_quality) + play_episode.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    export ANI_CLI_PLAYER='debug'

    # Build a fixture directory the curl shim can read from.
    export CURL_FIXTURE_DIR="$BATS_TEST_TMPDIR/fixtures"
    mkdir -p "$CURL_FIXTURE_DIR"
    cp "$FIXTURES_DIR/allanime/search_one_piece.json" "$CURL_FIXTURE_DIR/"
    cp "$FIXTURES_DIR/allanime/episodes_short.json" "$CURL_FIXTURE_DIR/"
    cp "$FIXTURES_DIR/allanime/embed_simple.json" "$CURL_FIXTURE_DIR/"
    bash "$REPO_ROOT/tests/bash/helpers/blob_builder.sh" "$CURL_FIXTURE_DIR/episode_blob.json"

    # Place curl shim on PATH.
    export PATH_SHIM="$BATS_TEST_TMPDIR/bin"
    mkdir -p "$PATH_SHIM"
    cp "$REPO_ROOT/tests/bash/helpers/curl_shim.sh" "$PATH_SHIM/curl"
    chmod +x "$PATH_SHIM/curl"
    export PATH="$PATH_SHIM:$PATH"
}

@test "search-and-play (debug player): prints 'All links:' and 'Selected link:' with the wixmp URL" {
    run "$ANI_CLI_PATH" -S 1 -e 1 -q best "test"
    [ "$status" -eq 0 ]
    [[ "$output" == *"All links:"* ]]
    [[ "$output" == *"Selected link:"* ]]
    # The wixmp embed fixture has resolution 720 and link wixmp.example/video.mp4.
    [[ "$output" == *"720 >https://wixmp.example/video.mp4"* ]]
    [[ "$output" == *"https://wixmp.example/video.mp4"* ]]
}

@test "search-and-play: writes the played episode to the history file" {
    run "$ANI_CLI_PATH" -S 1 -e 1 -q best "test"
    [ "$status" -eq 0 ]
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    [ -s "$histfile" ]
    # The first edge in search_one_piece.json is "ReooPAxPMsHM4KPMY"/One Piece.
    grep -E "^1"$'\t'"ReooPAxPMsHM4KPMY"$'\t'"One Piece" "$histfile" >/dev/null
}

@test "continue-from-history (-c): replays the next episode for an in-progress entry" {
    # Pre-populate history with episode 1 watched of a show whose episodes
    # list (per the shim) is [1, 1.5, 2, 3]. The continue path should pick
    # the next-episode (1.5) for replay.
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    printf '1\tReooPAxPMsHM4KPMY\tOne Piece (3 episodes)\n' >"$histfile"
    run "$ANI_CLI_PATH" -c -S 1 -q best
    [ "$status" -eq 0 ]
    [[ "$output" == *"Selected link:"* ]]
    # History now points at episode 1.5 (the next after 1).
    grep -E '^1\.5'$'\t'"ReooPAxPMsHM4KPMY" "$histfile" >/dev/null
}

@test "search-and-play with --dub: argv is honored even when curl shim is mode-agnostic" {
    # Confirms --dub flips the script's mode internally without aborting the
    # pipeline; the shim's blob is reused so playback still resolves.
    run "$ANI_CLI_PATH" --dub -S 1 -e 1 -q best "test"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Selected link:"* ]]
}

@test "download-flag: -d invokes the download path (aria2c via the shim)" {
    # We need aria2c on PATH or stubbed for this test. Place a stub under
    # PATH_SHIM so the download function call records and exits 0.
    cat >"$PATH_SHIM/aria2c" <<'EOF'
#!/bin/sh
printf 'aria2c %s\n' "$*" >"$BATS_TEST_TMPDIR/aria2c.log"
exit 0
EOF
    chmod +x "$PATH_SHIM/aria2c"
    # ffmpeg is also dep-checked in download mode; provide a no-op stub.
    cat >"$PATH_SHIM/ffmpeg" <<'EOF'
#!/bin/sh
exit 0
EOF
    chmod +x "$PATH_SHIM/ffmpeg"

    run "$ANI_CLI_PATH" -d -S 1 -e 1 -q best "test"
    [ "$status" -eq 0 ]
    # aria2c was invoked with the wixmp URL.
    [ -f "$BATS_TEST_TMPDIR/aria2c.log" ]
    grep -q 'https://wixmp.example/video.mp4' "$BATS_TEST_TMPDIR/aria2c.log"
}
