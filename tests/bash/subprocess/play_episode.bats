#!/usr/bin/env bats
#
# Tests for ani-cli's `play_episode` (lines 384-431).
#
# Contract:
#   - Reads many globals (player_function, episode, ep_no, id, title,
#     allanime_title, subs_flag, refr_flag, no_detach, exit_after_play,
#     skip_intro, log_episode, replay, range, use_external_menu).
#   - Branches on player_function and invokes the right player.
#   - Updates history after launching.
#
# We stub every external command play_episode might call (nohup, mpv, vlc,
# am, flatpak, catt, logger, ani-skip) and assert which one was invoked
# with which args.

load '../helpers/loader'

setup() {
    # History must be redirected to a tmp dir BEFORE sourcing — the script
    # initializes $histfile during setup.
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/process_stub'
    stub_setup
    stub_command nohup mpv vlc flatpak am catt logger ani-skip syncplay yt-dlp ffmpeg aria2c

    # Common state every branch reads.
    ep_no='1'
    id='showid'
    title='Test Anime (1 episodes)'
    allanime_title='Test Anime '
    episode='https://example.com/video.mp4'
    subs_flag=''
    refr_flag=''
    skip_intro=0
    log_episode=0
    no_detach=0
    exit_after_play=0
    use_external_menu=0
    range=''
}

@test "play_episode: mpv default branch invokes nohup mpv with --force-media-title" {
    player_function='mpv'
    play_episode
    wait 2>/dev/null || true   # allow backgrounded stubs to flush
    stub_assert_called nohup '.*mpv.*--force-media-title=Test Anime Episode 1.*https://example.com/video.mp4'
}

@test "play_episode: mpv with no_detach=1 calls mpv synchronously (no nohup)" {
    player_function='mpv'
    no_detach=1
    play_episode
    wait 2>/dev/null || true
    stub_assert_called mpv '.*--force-media-title=Test Anime Episode 1.*https://example.com/video.mp4'
    stub_assert_not_called nohup
}

@test "play_episode: vlc branch passes --http-referrer using allanime_refr" {
    player_function='vlc'
    play_episode
    wait 2>/dev/null || true
    stub_assert_called nohup '.*vlc.*--http-referrer=https://allmanga.to.*'
    stub_assert_called nohup '.*--meta-title=Test Anime Episode 1.*'
}

@test "play_episode: flatpak_mpv branch invokes flatpak run io.mpv.Mpv" {
    player_function='flatpak_mpv'
    play_episode
    wait 2>/dev/null || true
    stub_assert_called flatpak '.*run io.mpv.Mpv .*--force-media-title=Test Anime Episode 1.*https://example.com/video.mp4'
}

@test "play_episode: catt branch invokes catt cast" {
    player_function='catt'
    subtitle='https://example.com/sub.vtt'
    play_episode
    wait 2>/dev/null || true
    stub_assert_called nohup '.*catt cast https://example.com/video.mp4 -s https://example.com/sub.vtt'
}

@test "play_episode: android_mpv branch invokes am start with MPVActivity" {
    player_function='android_mpv'
    play_episode
    wait 2>/dev/null || true
    stub_assert_called nohup '.*am start.*is.xyz.mpv/.MPVActivity.*'
}

@test "play_episode: android_vlc branch invokes am start with VideoPlayerActivity" {
    player_function='android_vlc'
    play_episode
    wait 2>/dev/null || true
    stub_assert_called nohup '.*am start.*org.videolan.vlc/org.videolan.vlc.gui.video.VideoPlayerActivity.*'
}

@test "play_episode: debug branch prints links and selected URL to stdout (no players invoked)" {
    player_function='debug'
    links=$'1080 >https://example.com/v1.mp4\n720 >https://example.com/v2.mp4'
    output=$(play_episode 2>/dev/null)
    [[ "$output" == *"All links:"* ]]
    [[ "$output" == *"1080 >https://example.com/v1.mp4"* ]]
    [[ "$output" == *"Selected link:"* ]]
    [[ "$output" == *"https://example.com/video.mp4"* ]]
    stub_assert_not_called nohup
    stub_assert_not_called mpv
}

@test "play_episode: updates the history file with ep_no\\tid\\ttitle" {
    player_function='mpv'
    play_episode
    wait 2>/dev/null || true
    grep -E "^1"$'\t'"showid"$'\t'"Test Anime" "$histfile" >/dev/null
}

@test "play_episode: log_episode=1 invokes logger with allanime_title and ep_no" {
    player_function='mpv'
    log_episode=1
    play_episode
    wait 2>/dev/null || true
    stub_assert_called logger '.*-t ani-cli Test Anime 1'
}
