#!/usr/bin/env bats
#
# Unit tests for ani-cli's `select_quality` (lines 194-214).
#
# Contract (input via globals, output via globals):
#   Inputs:  $links (newline-separated "WIDTH >URL" lines), $player_function.
#   Outputs: $episode (the chosen URL), optionally $subs_flag, $refr_flag,
#            $subtitle, $m3u8_refr (set/unset based on link kind).
#
# Quality argument:
#   - "best"  → first line of $links (already sorted descending in production).
#   - "worst" → last line matching `^[0-9]{3,4}` (numeric quality only).
#   - other   → first line matching the literal string.
#   - not found → falls back to best, prints a warning to stderr.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # Reset all globals select_quality interacts with
    unset episode subs_flag refr_flag subtitle m3u8_refr
    player_function='mpv'
}

@test "select_quality: 'best' picks the first link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "best"
    [ "$episode" = "https://a.example/1080.mp4" ]
}

@test "select_quality: 'worst' picks the last numeric-quality link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "worst"
    [ "$episode" = "https://a.example/480.mp4" ]
}

@test "select_quality: explicit '1080' picks the 1080 link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "1080"
    [ "$episode" = "https://a.example/1080.mp4" ]
}

@test "select_quality: explicit '720' picks the 720 link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "720"
    [ "$episode" = "https://a.example/720.mp4" ]
}

@test "select_quality: not-found falls back to best with stderr warning" {
    links=$'1080 >https://a.example/1080.mp4\n480 >https://a.example/480.mp4'
    output=$(select_quality "9999" 2>&1 >/dev/null)
    # episode set to best
    [ "$episode" = "https://a.example/1080.mp4" ]
    [[ "$output" =~ "Specified quality not found" ]]
}

@test "select_quality: vlc strips m3u8-cc/subtitle/refr metadata lines before picking" {
    player_function='vlc'
    links=$'1080cc>https://a.example/1080.m3u8\n720 >https://a.example/720.mp4\nsubtitle >https://a.example/sub.vtt\nm3u8_refr >https://allmanga.to'
    select_quality "best"
    # vlc filter removes /cc>/d, /subtitle >/d, /m3u8_refr >/d → first remaining is 720.
    [ "$episode" = "https://a.example/720.mp4" ]
}

@test "select_quality: m3u8 (cc>) sets refr_flag and subs_flag" {
    player_function='mpv'
    links=$'1080cc>https://a.example/1080.m3u8\nsubtitle >https://a.example/sub.vtt\nm3u8_refr >https://allmanga.to'
    select_quality "best"
    [ "$episode" = "https://a.example/1080.m3u8" ]
    [ "$subs_flag" = "--sub-file=https://a.example/sub.vtt" ]
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}

@test "select_quality: tools.fast4speed link sets refr_flag to allanime_refr" {
    player_function='mpv'
    allanime_refr='https://allmanga.to'
    links=$'1080 >https://tools.fast4speed.rsvp/path'
    select_quality "best"
    [ "$episode" = "https://tools.fast4speed.rsvp/path" ]
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}

@test "select_quality: mp4 (no cc>) leaves subs_flag/refr_flag unset" {
    player_function='mpv'
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4'
    select_quality "best"
    [ "$episode" = "https://a.example/1080.mp4" ]
    [ -z "${subs_flag-}" ]
    [ -z "${refr_flag-}" ]
}
