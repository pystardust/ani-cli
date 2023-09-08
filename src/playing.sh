#!/bin/sh
# PLAYING

process_hist_entry() {
    ep_list=$(episodes_list "$id")
    ep_no=$(printf "%s" "$ep_list" | sed -n "/^${ep_no}$/{n;p;}") 2>/dev/null
    [ -n "$ep_no" ] && printf "%s\t%s - episode %s\n" "$id" "$title" "$ep_no"
}

update_history() {
    if grep -q -- "$id" "$histfile"; then
        sed -E "s/^[^\t]+\t${id}\t/${ep_no}\t${id}\t/" "$histfile" >"${histfile}.new"
    else
        cp "$histfile" "${histfile}.new"
        printf "%s\t%s\t%s\n" "$ep_no" "$id" "$title" >>"${histfile}.new"
    fi
    mv "${histfile}.new" "$histfile"
}

download() {
    case $1 in
        *m3u8*)
            if command -v "yt-dlp" >/dev/null; then
                yt-dlp "$1" --no-skip-unavailable-fragments --fragment-retries infinite -N 16 -o "$download_dir/$2.mp4"
            else
                ffmpeg -loglevel error -stats -i "$1" -c copy "$download_dir/$2.mp4"
            fi
            ;;
        *)
            aria2c --enable-rpc=false --check-certificate=false --continue --summary-interval=0 -x 16 -s 16 "$1" --dir="$download_dir" -o "$2.mp4" --download-result=hide
            ;;
    esac
}

play_episode() {
    [ -z "$episode" ] && get_episode_url
    case "$player_function" in
        debug)
            [ -z "$ANI_CLI_NON_INTERACTIVE" ] && printf "All links:\n%s\nSelected link:\n" "$links"
            printf "%s\n" "$episode"
            ;;
        mpv*) nohup "$player_function" --force-media-title="${allanime_title}Episode ${ep_no}" "$episode" >/dev/null 2>&1 & ;;
        android_mpv) nohup am start --user 0 -a android.intent.action.VIEW -d "$episode" -n is.xyz.mpv/.MPVActivity >/dev/null 2>&1 & ;;
        android_vlc) nohup am start --user 0 -a android.intent.action.VIEW -d "$episode" -n org.videolan.vlc/org.videolan.vlc.gui.video.VideoPlayerActivity -e "title" "${allanime_title}Episode ${ep_no}" >/dev/null 2>&1 & ;;
        iina) nohup "$player_function" --no-stdin --keep-running --mpv-force-media-title="${allanime_title}Episode ${ep_no}" "$episode" >/dev/null 2>&1 & ;;
        flatpak_mpv) flatpak run io.mpv.Mpv --force-media-title="${allanime_title}Episode ${ep_no}" "$episode" >/dev/null 2>&1 & ;;
        vlc*) nohup "$player_function" --play-and-exit --meta-title="${allanime_title}Episode ${ep_no}" "$episode" >/dev/null 2>&1 & ;;
        *yncpla*) nohup "$player_function" "$episode" -- --force-media-title="${allanime_title}Episode ${ep_no}" >/dev/null 2>&1 & ;;
        download) "$player_function" "$episode" "${allanime_title}Episode ${ep_no}" ;;
        catt) nohup catt cast "$episode" >/dev/null 2>&1 & ;;
        iSH)
            printf "\e]8;;vlc://%s\a~~~~~~~~~~~~~~~~~~~~\n~ Tap to open VLC ~\n~~~~~~~~~~~~~~~~~~~~\e]8;;\a\n" "$episode"
            sleep 5
            ;;
        *) nohup "$player_function" "$episode" >/dev/null 2>&1 & ;;
    esac
    replay="$episode"
    unset episode
    update_history
    [ "$use_external_menu" = "1" ] && wait
}

play() {
    start=$(printf "%s" "$ep_no" | grep -Eo '^(-1|[0-9]+(\.[0-9]+)?)')
    end=$(printf "%s" "$ep_no" | grep -Eo '(-1|[0-9]+(\.[0-9]+)?)$')
    [ "$start" = "-1" ] && ep_no=$(printf "%s" "$ep_list" | tail -n1) && unset start
    [ -z "$end" ] || [ "$end" = "$start" ] && unset start end
    [ "$end" = "-1" ] && end=$(printf "%s" "$ep_list" | tail -n1)
    line_count=$(printf "%s\n" "$ep_no" | wc -l | tr -d "[:space:]")
    if [ "$line_count" != 1 ] || [ -n "$start" ]; then
        [ -z "$start" ] && start=$(printf "%s\n" "$ep_no" | head -n1)
        [ -z "$end" ] && end=$(printf "%s\n" "$ep_no" | tail -n1)
        range=$(printf "%s\n" "$ep_list" | sed -nE "/^${start}\$/,/^${end}\$/p")
        [ -z "$range" ] && die "Invalid range!"
        for i in $range; do
            tput clear
            ep_no=$i
            printf "\33[2K\r\033[1;34mPlaying episode %s...\033[0m\n" "$ep_no"
            play_episode
        done
    else
        play_episode
    fi
    # moves upto stored positon and deletes to end
    [ "$player_function" != "debug" ] && [ "$player_function" != "download" ] && tput rc && tput ed
}
