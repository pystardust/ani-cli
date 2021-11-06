#!/bin/sh

# dependencies: grep sed curl video_player
# video_player ( needs to be able to play urls )
player_fn="mpv"

prog="ani-cli"
logfile="${XDG_CACHE_HOME:-$HOME/.cache}/ani-hsts"
base_url="https://gogoanime.cm"

c_red="\033[1;31m"
c_green="\033[1;32m"
c_yellow="\033[1;33m"
c_blue="\033[1;34m"
c_magenta="\033[1;35m"
c_cyan="\033[1;36m"
c_reset="\033[0m"


help_text () {
	while IFS= read line; do
		printf "%s\n" "$line"
	done <<-EOF
	USAGE: $prog <query>
	 -h	 show this help text
	 -d	 download episode
	 -H	 continue where you left off
	 -D	 delete history
	 -q	 set video quality (best/worst/360/480/720/..)
	 --dub  play the dub version if present
	EOF
}


die () {
	printf "$c_red%s$c_reset\n" "$*" >&2
	exit 1
}

err () {
	printf "$c_red%s$c_reset\n" "$*" >&2
}

search_anime () {
	# get anime name along with its id
	search=$(printf '%s' "$1" | tr ' ' '-' )
	titlepattern='<a href="/category/'

	curl -s "$base_url//search.html" \
		-G \
		-d "keyword=$search" |
	sed -n -E '
		s_^[[:space:]]*<a href="/category/([^"]*)" title="([^"]*)".*_\1_p
		'
}

search_eps () {
	# get available episodes for anime_id
	anime_id=$1

	curl -s "$base_url/category/$anime_id" |
	sed -n -E '
		/^[[:space:]]*<a href="#" class="active" ep_start/{
		s/.* '\''([0-9]*)'\'' ep_end = '\''([0-9]*)'\''.*/\2/p
		q
		}
		'
}

get_embedded_video_link() {
	# get the download page url
	anime_id=$1
	ep_no=$2

	# credits to fork: https://github.com/Dink4n/ani-cli for the fix
	# dub prefix takes the value "-dub" when dub is needed else is empty
	curl -s "$base_url/$anime_id${dub_prefix}-episode-$ep_no" |
	sed -n -E '
		/^[[:space:]]*<a href="#" rel="100"/{
		s/.*data-video="([^"]*)".*/https:\1/p
		q
		}'
}

get_video_quality() {
	embedded_video_url=$1
	video_url=$2

	video_file=$(curl -s --referer "$embedded_video_url" "$video_url")
	available_qualities=$(printf '%s' "$video_file" | sed -n -E 's/.*NAME="([^p]*)p"/\1/p')
	case $quality in
		best)
			printf '%s' "$available_qualities" | tail -n 1
			;;

		worst)
			printf '%s' "$available_qualities" | head -n 1
			;;

		*)
			is_quality_avail=$(printf '%s' "$available_qualities" | grep "$quality")
			video_quality="$quality"
			if [ -z "$is_quality_avail" ]; then
				printf "$c_red%s$c_reset\n" "Current video quality is not available (defaulting to highest quality)" >&2
				quality=best
				video_quality=$(printf '%s' "$available_qualities" | tail -n 1)
			fi
			printf '%s' "$video_quality"
			;;
	esac

}

get_links () {
	embedded_video_url="$1"
	video_url=$(curl -s "$embedded_video_url" |
	sed -n -E '
		/^[[:space:]]*sources:/{
		s/.*(https[^'\'']*).*/\1/p
		q
		}
		')

	video_quality=$(get_video_quality "$embedded_video_url" "$video_url")

	# Replace the video with highest quality video
	printf '%s' "$video_url" | sed -n -E "s/(.*)\.m3u8/\1.$video_quality.m3u8/p"
}

dep_ch () {
	for dep; do
		if ! command -v "$dep" >/dev/null ; then
			die "Program \"$dep\" not found. Please install it."
		fi
	done
}

# get query
get_search_query () {
	if [ -z "$*" ]; then
		printf "Search Anime: "
		read -r query
	else
		query=$*
	fi
}

# create history file
[ -f "$logfile" ] || : > "$logfile"

#####################
## Anime selection ##
#####################

anime_selection () {
	search_results=$*
	menu_format_string='[%d] %s\n'
	menu_format_string_c1="$c_blue[$c_cyan%d$c_blue] $c_reset%s\n"
	menu_format_string_c2="$c_blue[$c_cyan%d$c_blue] $c_yellow%s$c_reset\n"

	count=1
	while read anime_id; do
		# alternating colors for menu
		[ $((count % 2)) -eq 0 ] &&
			menu_format_string=$menu_format_string_c1 ||
			menu_format_string=$menu_format_string_c2

		printf "$menu_format_string" "$count" "$anime_id"
		count=$((count+1))
	done <<-EOF
	$search_results
	EOF

	# User input
	printf "$c_blue%s$c_green" "Enter number: "
	read choice
	printf "$c_reset"

	# Check if input is a number
	[ "$choice" -eq "$choice" ] 2>/dev/null || die "Invalid number entered"

	# Select respective anime_id
	count=1
	while read anime_id; do
		if [ $count -eq $choice ]; then
			selection_id=$anime_id
			break
		fi
		count=$((count+1))
	done <<-EOF
	$search_results
	EOF

	[ -z "$selection_id" ] && die "Invalid number entered"

	read last_ep_number <<-EOF
	$(search_eps "$selection_id")
	EOF
}

##################
## Ep selection ##
##################

episode_selection () {
	ep_choice_start="1"
	if [ $last_ep_number -gt 1 ] 
	then
		[ $is_download -eq 1 ] &&
			printf "Range of episodes can be specified: start_number end_number\n"

		printf "${c_blue}Choose episode $c_cyan[1-%d]$c_reset:$c_green " $last_ep_number
		read ep_choice_start ep_choice_end
		printf "$c_reset"
	fi
}

check_input() {
	[ "$ep_choice_start" -eq "$ep_choice_start" ] 2>/dev/null || die "Invalid number entered"
	episodes=$ep_choice_start
	if [ -n "$ep_choice_end" ]; then
		[ "$ep_choice_end" -eq "$ep_choice_end" ] 2>/dev/null || die "Invalid number entered"
		# create list of episodes to download/watch
		episodes=$(seq $ep_choice_start $ep_choice_end)
	fi
}

append_history () {
	grep -q -w "${selection_id}" "$logfile" ||
		printf "%s\t%d\n" "$selection_id" $((episode+1)) >> "$logfile"
}

open_selection() {
	for ep in $episodes
	do
		open_episode "$selection_id" "$ep"
	done
	episode=${ep_choice_end:-$ep_choice_start}
}

open_episode () {
	anime_id=$1
	episode=$2

	# Cool way of clearing screen
	tput reset
	while [ "$episode" -lt 1 ] || [ "$episode" -gt "$last_ep_number" ]
	do
		err "Episode out of range"
		printf "${c_blue}Choose episode $c_cyan[1-%d]$c_reset:$c_green " $last_ep_number
		read episode
		printf "$c_reset"
	done

	printf "Getting data for episode %d\n" $episode

	embedded_video_url=$(get_embedded_video_link "$anime_id" "$episode")
	video_url=$(get_links "$embedded_video_url")

	case $video_url in
		*streamtape*)
			# If direct download not available then scrape streamtape.com
			BROWSER=${BROWSER:-firefox}
			printf "scraping streamtape.com\n"
			video_url=$(curl -s "$video_url" | sed -n -E '
				/^<script>document/{
				s/^[^"]*"([^"]*)" \+ '\''([^'\'']*).*/https:\1\2\&dl=1/p
				q
				}
			');;
	esac

	if [ $is_download -eq 0 ]; then
		# write anime and episode number
		sed -E "
			s/^${selection_id}\t[0-9]+/${selection_id}\t$((episode+1))/
		" "$logfile" > "${logfile}.new" && mv "${logfile}.new" "$logfile"

		setsid -f $player_fn --http-header-fields="Referer: $embedded_video_url" "$video_url" >/dev/null 2>&1
	else
		printf "Downloading episode $episode ...\n"
		printf "%s\n" "$video_url"
		# add 0 padding to the episode name
		episode=$(printf "%03d" $episode)
		{
			ffmpeg -headers "Referer: $embedded_video_url" -i "$video_url" \
				-c copy "${anime_id}-${episode}.mkv" >/dev/null 2>&1 &&
				printf "${c_green}Downloaded episode: %s${c_reset}\n" "$episode" ||
				printf "${c_red}Download failed episode: %s${c_reset}\n" "$episode"
		}
	fi
}

############
# Start Up #
############

# to clear the colors when exited using SIGINT
trap "printf '$c_reset'" INT HUP

dep_ch "$player_fn" "curl" "sed" "grep"

# option parsing
is_download=0
quality=best
scrape=query
while getopts 'hdHDq:-:' OPT; do
	case $OPT in
		h)
			help_text
			exit 0
			;;
		d)
			is_download=1
			;;
		H)
			scrape=history
			;;

		D)
			: > "$logfile"
			exit 0
			;;
		q)
			quality=$OPTARG
			;;
		-)
			case $OPTARG in
				dub)
					dub_prefix="-dub"
					;;
			esac
			;;
	esac
done
shift $((OPTIND - 1))

########
# main #
########

case $scrape in
	query)
		get_search_query "$*"
		search_results=$(search_anime "$query")
		[ -z "$search_results" ] && die "No search results found"
		anime_selection "$search_results"
		episode_selection
		;;
	history)
		search_results=$(sed -n -E 's/\t[0-9]*//p' "$logfile")
		[ -z "$search_results" ] && die "History is empty"
		anime_selection "$search_results"
		ep_choice_start=$(sed -n -E "s/${selection_id}\t//p" "$logfile")
		;;
esac

check_input
append_history
open_selection

while :; do
	printf "\n${c_green}Currently playing %s episode ${c_cyan}%d/%d\n" "$selection_id" $episode $last_ep_number
	if [ "$episode" -ne "$last_ep_number" ]; then
		printf "$c_blue[${c_cyan}%s$c_blue] $c_yellow%s$c_reset\n" "n" "next episode"
	fi
	if [ "$episode" -ne "1" ]; then
		printf "$c_blue[${c_cyan}%s$c_blue] $c_magenta%s$c_reset\n" "p" "previous episode"
	fi
	if [ "$last_ep_number" -ne "1" ]; then
		printf "$c_blue[${c_cyan}%s$c_blue] $c_yellow%s$c_reset\n" "s" "select episode"
	fi
	printf "$c_blue[${c_cyan}%s$c_blue] $c_magenta%s$c_reset\n" "r" "replay current episode"
	printf "$c_blue[${c_cyan}%s$c_blue] $c_cyan%s$c_reset\n" "a" "search for another anime"
	printf "$c_blue[${c_cyan}%s$c_blue] $c_red%s$c_reset\n" "q" "exit"
	printf "${c_blue}Enter choice:${c_green} "
	read choice
	printf "$c_reset"
	case $choice in
		n)
			episode=$((episode + 1))
			;;
		p)
			episode=$((episode - 1))
			;;

		s)	printf "${c_blue}Choose episode $c_cyan[1-%d]$c_reset:$c_green " $last_ep_number
			read episode
			printf "$c_reset"
			[ "$episode" -eq "$episode" ] 2>/dev/null || die "Invalid number entered"
			;;

		r)
			episode=$((episode))
			;;
		a)
			tput reset
			get_search_query ""
			search_results=$(search_anime "$query")
			[ -z "$search_results" ] && die "No search results found"
			anime_selection "$search_results"
			episode_selection
			check_input
			append_history
			open_selection
			continue
			;;

		q)
			break;;

		*)
			die "invalid choice"
			;;
	esac

	open_episode "$selection_id" "$episode"
done
