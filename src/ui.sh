#!/bin/sh
# UI

external_menu() {
    rofi "$1" -sort -dmenu -i -width 1500 -p "$2"
}

launcher() {
    [ "$use_external_menu" = "0" ] && [ -z "$1" ] && set -- "+m" "$2"
    [ "$use_external_menu" = "0" ] && fzf "$1" --reverse --cycle --prompt "$2"
    [ "$use_external_menu" = "1" ] && external_menu "$1" "$2"
}

nth() {
    stdin=$(cat -)
    [ -z "$stdin" ] && return 1
    line_count="$(printf "%s\n" "$stdin" | wc -l | tr -d "[:space:]")"
    [ "$line_count" -eq 1 ] && printf "%s" "$stdin" | cut -f2,3 && return 0
    prompt="$1"
    multi_flag=""
    [ $# -ne 1 ] && shift && multi_flag="$1"
    line=$(printf "%s" "$stdin" | cut -f1,3 | tr '\t' ' ' | launcher "$multi_flag" "$prompt" | cut -d " " -f 1)
    [ -n "$line" ] && printf "%s" "$stdin" | grep -E '^'"${line}"'($|\s)' | cut -f2,3 || exit 1
}

die() {
    printf "\33[2K\r\033[1;31m%s\033[0m\n" "$*" >&2
    exit 1
}

help_info() {
    printf "
    Usage:
    %s [options] [query]
    %s [query] [options]
    %s [options] [query] [options]

    Options:
      -c, --continue
        Continue watching from history
      -d, --download
        Download the video instead of playing it
      -D, --delete
        Delete history
      -s, --syncplay
        Use Syncplay to watch with friends
      -S, --select-nth
        Select nth entry
      -q, --quality
        Specify the video quality
      -v, --vlc
        Use VLC to play the video
      -V, --version
        Show the version of the script
      -h, --help
        Show this help message and exit
      -e, --episode, -r, --range
        Specify the number of episodes to watch
      --dub
        Play dubbed version
      --rofi
        Use rofi instead of fzf for the interactive menu
      -U, --update
        Update the script
    Some example usages:
      %s -q 720p banana fish
      %s -d -e 2 cyberpunk edgerunners
      %s --vlc cyberpunk edgerunners -q 1080p -e 4
      %s blue lock -e 5-6
      %s -e \"5 6\" blue lock
    \n" "${0##*/}" "${0##*/}" "${0##*/}" "${0##*/}" "${0##*/}" "${0##*/}" "${0##*/}" "${0##*/}"
    exit 0
}

version_info() {
    printf "%s\n" "$version_number"
    exit 0
}

update_script() {
    update="$(curl -s -A "$agent" "https://raw.githubusercontent.com/pystardust/ani-cli/master/ani-cli")" || die "Connection error"
    update="$(printf '%s\n' "$update" | diff -u "$0" -)"
    if [ -z "$update" ]; then
        printf "Script is up to date :)\n"
    else
        if printf '%s\n' "$update" | patch "$0" -; then
            printf "Script has been updated\n"
        else
            die "Can't update for some reason!"
        fi
    fi
    exit 0
}

# checks if dependencies are present
dep_ch() {
    for dep; do
        command -v "$dep" >/dev/null || die "Program \"$dep\" not found. Please install it."
    done
}