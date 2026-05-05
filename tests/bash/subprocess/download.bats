#!/usr/bin/env bats
#
# Tests for ani-cli's `download` (lines 364-382).
#
# Contract:
#   - $1 = stream URL (mp4 or m3u8), $2 = base name (no extension), and the
#     env var $subtitle if a subtitle URL was found.
#   - If subtitle is set, fetch it via curl into $download_dir/$2.vtt.
#   - Branches on $1:
#       *m3u8*: yt-dlp if available, else ffmpeg fallback.
#       *     : aria2c.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/process_stub'
    stub_setup
    stub_command yt-dlp ffmpeg aria2c
    # curl is used to fetch the subtitle; stub it but make it a no-op so the
    # script continues. The download function does `curl -s "$subtitle" -o ...`
    # — we don't need to actually create the .vtt file for these tests.
    curl() { return 0; }
    export -f curl

    download_dir="$BATS_TEST_TMPDIR/dl"
    mkdir -p "$download_dir"
    iSH_DownFix=''
    allanime_refr='https://allmanga.to'
    subtitle=''
}

@test "download: mp4 url with aria2c records expected args" {
    download "https://example.com/video.mp4" "Test Anime Episode 1"
    stub_assert_called aria2c '.*--referer=https://allmanga.to.*--continue.*-x 16.*-s 16.*https://example.com/video.mp4'
    stub_assert_called aria2c ".*--dir=$download_dir.*-o Test Anime Episode 1.mp4.*"
}

@test "download: m3u8 url with yt-dlp present uses yt-dlp" {
    m3u8_refr='https://allmanga.to'
    download "https://example.com/master.m3u8" "Test Anime Episode 1"
    stub_assert_called yt-dlp '.*--referer https://allmanga.to.*--no-skip-unavailable-fragments.*-N 16.*'
    stub_assert_called yt-dlp ".*-o $download_dir/Test Anime Episode 1.mp4"
    stub_assert_not_called ffmpeg
    stub_assert_not_called aria2c
}

@test "download: m3u8 url falls back to ffmpeg when yt-dlp is missing" {
    # Simulate yt-dlp absent by overriding command -v to return 1 for it.
    command() {
        if [ "$1" = "-v" ] && [ "$2" = "yt-dlp" ]; then return 1; fi
        builtin command "$@"
    }
    export -f command
    m3u8_refr='https://allmanga.to'
    download "https://example.com/master.m3u8" "Test Anime Episode 1"
    stub_assert_called ffmpeg '.*-referer https://allmanga.to.*-i https://example.com/master.m3u8.*-c copy.*'
    stub_assert_called ffmpeg ".*$download_dir/Test Anime Episode 1.mp4"
    stub_assert_not_called yt-dlp
    stub_assert_not_called aria2c
}

@test "download: subtitle URL is fetched into download_dir/<basename>.vtt" {
    # Track the curl call by re-defining curl as a stub.
    curl_called=''
    curl() { curl_called="$*"; }
    export -f curl
    subtitle='https://example.com/sub.vtt'
    download "https://example.com/video.mp4" "Test Anime Episode 1"
    [[ "$curl_called" == *"-s https://example.com/sub.vtt -o $download_dir/Test Anime Episode 1.vtt"* ]]
}

@test "download: no subtitle env var skips the curl fetch" {
    curl_called='not-called'
    curl() { curl_called="$*"; }
    export -f curl
    subtitle=''
    download "https://example.com/video.mp4" "Test Anime Episode 1"
    [ "$curl_called" = 'not-called' ]
}
