#!/bin/sh

# version number
VERSION="1.5.5"


# history file path
logfile="${XDG_CACHE_HOME:-$HOME/.cache}/ani-hsts"
auto_play=0

#######################
# Auxiliary functions #
#######################

help_text () {
	while IFS= read -r line; do
		printf "%s\n" "$line"
	done <<-EOF

	Usage:
	  $0 [-v | -i] [-q <quality>] [-e <arguments>] [-a <episode>] [-d | -p <download_dir>] [<query>]
	  $0 [-v | -i] [-q <quality>] [-e <arguments>] -c
	  $0 -h | -D | -U | -V

	Options:
	  -c continue watching anime from history
	  -e pass arguments to the player/downloader
	  -a specify episode to watch
	  -h show helptext
	  -d download episode
	  -p download episode to specified directory
	  -q set video quality (best|worst|360|480|720|1080)
	  -i use iina as the media player
	  -v use VLC as the media player
	  -D delete history
	  -U fetch update from github
	  -V print version number and exit

	Episode selection:
	  Add 'h' on beginning for episodes like '6.5' -> 'h6'
	  Multiple episodes can be chosen given a range
	    Choose episode [1-13]: 1 6
	    This would choose episodes 1 2 3 4 5 6
	  When selecting non-interactively, the first result will be
	  selected, if anime is passed

	Passing arguments:
	  Put all the arguments inside quotes '' or ""
	  Seperate the arguments with spaces
	  For e.g.
	  	ani-cli -e '--brightness=-15 --gamma=-15 --pause'
	EOF
}

version_text () {
	inf "Version: $VERSION"  >&2
}

die () {
	err "$*"
	exit 1
}

update_script () {
	# get the newest version of this script from github and replace it
	update="$(curl -s "https://raw.githubusercontent.com/pystardust/ani-cli/master/ani-cli" | diff -u "$0" -)"
	if [ -z "$update" ]; then
		inf "Script is up to date :)"
	else
		if printf '%s\n' "$update" | patch "$0" - ; then
			inf "Script has been updated"
		else
			die "Can't update for some reason!"
		fi
	fi
}

dep_ch () {
	# checks if programs are present
	for dep; do
		if ! command -v "$dep" >/dev/null ; then
			err "Program \"$dep\" not found. Please install it."
			#aria2c is in the package aria2
			if [ "$dep" = "aria2c" ]; then
				err "To install aria2c, Type <your_package_manager> aria2"
			fi
			die
		fi
	done
}


#############
# Searching #
#############

search_anime () {
	# get anime name along with its id for search term
	search=$(printf '%s' "$1" | tr ' ' '-' )

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

search_for_unwatched () {
	# compares history with gogoanime, only shows unfinished anime
	search_results=$*

	unwatched_anime=""
	while read -r anime_id; do
		current_ep_number="$(search_eps "$anime_id")"
		history_ep_number="$(sed -n -E "s/${anime_id}\t//p" "$logfile")"
		if [ "$current_ep_number" -ge "$history_ep_number" ]
		then
			# breaks down with \n because of the display functions
			unwatched_anime="$unwatched_anime$anime_id
"
		fi
	done <<-EOF
	$search_results
	EOF
	if [ -z "$unwatched_anime" ]; then
		die "No unwatched episodes"
	fi
	printf "%s" "$unwatched_anime"
}


##################
# URL processing #
##################

get_dpage_link() {
	# get the download page url
	anime_id=$1
	ep_no=$2

	# credits to fork: https://github.com/Dink4n/ani-cli for the fix
	# dub prefix takes the value "-dub" when dub is needed else is empty
	anime_page=$(curl -s "$base_url/$anime_id-$ep_no")

	if printf '%s' "$anime_page" | grep -q "404" ; then
		anime_page=$(curl -s "$base_url/$anime_id-episode-$ep_no")
	fi

	printf '%s' "$anime_page" |
		sed -n -E 's/^[[:space:]]*<a href="#" rel="100" data-video="([^"]*)".*/\1/p' |
		sed 's/^/https:/g'
}

decrypt_link() {
	ajax_url='https://gogoplay.io/encrypt-ajax.php'

	#get the id from the url
	video_id=$(echo "$1" | cut -d\? -f2 | cut -d\& -f1 | sed 's/id=//g')

	#construct ajax parameters
	secret_key='3235373436353338353932393338333936373634363632383739383333323838'
	iv='34323036393133333738303038313335'
	ajax=$(echo "$video_id" | openssl enc -aes256  -K "$secret_key" -iv "$iv" -a)

	#send the request to the ajax url
	curl -s -H 'x-requested-with:XMLHttpRequest' "$ajax_url" -d "id=$ajax" -d "time=69420691337800813569" |
	sed -e 's/\].*/\]/' -e 's/\\//g' |
	grep -Eo 'https:\/\/[-a-zA-Z0-9@:%._\+~#=][a-zA-Z0-9][-a-zA-Z0-9@:%_\+.~#?&\/\/=]*'
}

get_video_quality() {
	# chooses the link for the set quality
	dpage_url="$1"
	video_links=$(decrypt_link "$dpage_url")
	case $quality in
		best)
			video_link=$(printf '%s' "$video_links" | head -n 4 | tail -n 1)
			;;

		worst)
			video_link=$(printf '%s' "$video_links" | head -n 1)
			;;

		*)
			video_link=$(printf '%s' "$video_links" | grep -i "${quality}p" | head -n 1)
			if [ -z "$video_link" ]; then
				err "Current video quality is not available (defaulting to best quality)"
				quality=best
				video_link=$(printf '%s' "$video_links" | head -n 4 | tail -n 1)
			fi
			;;
	esac
	printf '%s' "$video_link"
}


###############
# Text output #
###############

err () {
	# display an error message to stderr (in red)
	printf "\033[1;31m%s\033[0m\n" "$*" >&2
}

inf () {
	# display an informational message (first argument in green, second in magenta)
	printf "\033[1;32m%s \033[1;35m%s\033[0m\n" "$1" "$2"
}

prompt () {
        # prompts the user with message in $1-2 ($1 in blue, $2 in magenta) and saves the input to the variables in $REPLY and $REPLY2
        printf "\033[1;34m%s\033[1;35m%s\033[1;34m: \033[0m" "$1" "$2"
        read -r REPLY REPLY2
}

menu_line_even () {
	# displays an even (cyan) line of a menu line with $2 as an indicator in [] and $1 as the option
	printf "\033[1;34m[\033[1;36m%s\033[1;34m] \033[1;36m%s\033[0m\n" "$2" "$1"
}

menu_line_odd() {
	# displays an odd (yellow) line of a menu line with $2 as an indicator in [] and $1 as the option
	printf "\033[1;34m[\033[1;33m%s\033[1;34m] \033[1;33m%s\033[0m\n" "$2" "$1"
}

menu_line_alternate() {
	menu_line_parity=${menu_line_parity:-0}

	if [ "$menu_line_parity" -eq 0 ]; then
		menu_line_odd "$1" "$2"
		menu_line_parity=1
	else
		menu_line_even "$1" "$2"
		menu_line_parity=0
	fi
}

menu_line_strong() {
	# displays a warning (red) line of a menu line with $2 as an indicator in [] and $1 as the option
	printf "\033[1;34m[\033[1;31m%s\033[1;34m] \033[1;31m%s\033[0m\n" "$2" "$1"
}


#################
# Input parsing #
#################

anime_selection () {
	count=1
	while read -r anime_id; do
		menu_line_alternate "$anime_id" "$count"
		: $((count+=1))
	done <<-EOF
	$search_results
	EOF
	if [ -n "$ep_choice_to_start" ] && [ -n "$select_first" ]; then
		tput reset
		choice=1
	elif [ -z "$ep_choice_to_start" ] || { [ -n "$ep_choice_to_start" ] && [ -z "$select_first" ]; }; then
		prompt "Enter number"
		choice="$REPLY $REPLY2"
	fi

		# Check if input is a number
		[ "$choice" -eq "$choice" ] 2>/dev/null || die "Invalid number entered"

		# Select respective anime_id
		count=1
		while read -r anime_id; do
			if [ "$count" -eq "$choice" ]; then
				selection_id=$anime_id
				break
			fi
			count=$((count+1))
		done <<-EOF
		$search_results
		EOF

	if [ -z "$selection_id" ]; then
		die "Invalid number entered"
	fi

	search_ep_result="$(search_eps "$selection_id")"
	read -r last_ep_number <<-EOF
	$search_ep_result
	EOF
}

episode_selection () {
	# using get_dpage_link to get confirmation from episode 0 if it exists,else first_ep_number becomes "1"
	first_ep_number="0"
	result=$(get_dpage_link "$anime_id" "$first_ep_number")

	if [ -n "$result" ]; then
		true
	else
		first_ep_number="1"
	fi

	if [ "$last_ep_number" -gt "$first_ep_number" ]; then

		inf "Range of episodes can be specified: start_number end_number"

		if [ -z "$ep_choice_to_start" ]; then
			prompt "Choose episode" "[$first_ep_number-$last_ep_number]"
			ep_choice_start=$REPLY
			ep_choice_end=$REPLY2
		else
			ep_choice_start=$ep_choice_to_start && unset ep_choice_to_start
		fi
		whether_half="$(echo "$ep_choice_start" | cut -c1-1)"
		if [ "$whether_half" = "h" ]
		then
			half_ep=1
			ep_choice_start="$(echo "$ep_choice_start" | cut -c2-)"
		fi
	else
		# In case the anime contains only a single episode
		ep_choice_start=1
	fi

	if [ -z "$ep_choice_end" ]; then
		auto_play=0
	else
		auto_play=1
	fi


}

check_input() {
	# checks if input is number, creates $episodes from $ep_choice_start and $ep_choice_end
	[ "$ep_choice_start" -eq "$ep_choice_start" ] 2>/dev/null || die "Invalid number entered"
	episodes=$ep_choice_start
	if [ -n "$ep_choice_end" ]; then
		[ "$ep_choice_end" -eq "$ep_choice_end" ] 2>/dev/null || die "Invalid number entered"
		# create list of episodes to download/watch
		episodes=$(seq "$ep_choice_start" "$ep_choice_end")
	fi
}


##################
# Video Playback #
##################

append_history () {
	grep -q -w "${selection_id}" "$logfile" ||
		printf "%s\t%d\n" "$selection_id" $((episode+1)) >> "$logfile"
}

open_selection() {
	# opens selected episodes one-by-one
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
	# checking if episode is in range
	while  [ "$episode" -gt "$last_ep_number" ] || [ -z "$episode"  ]
	do
		[ "$ep_choice_start" -eq "$ep_choice_start" ] 2>/dev/null || die "Invalid number entered"
		if [ "$last_ep_number" -eq 0 ]; then
			die "Episodes not released yet!"
		else
			err "Episode out of range"
		fi
		prompt "Choose episode" "[$first_ep_number-$last_ep_number]"
		episode="$REPLY $REPLY2"
	done
	#processing half episodes
	if [ "$half_ep" -eq 1 ]
	then
		temp_ep=$episode
		episode=$episode"-5"
	fi

	inf "Getting data for episode $episode"
	# decrypting url
	dpage_link=$(get_dpage_link "$anime_id" "$episode")
	video_url=$(get_video_quality "$dpage_link")
	if [ "$half_ep" -eq 1 ]; then
		episode=$temp_ep
		half_ep=0
	fi
	# Download or play episodes
	if [ "$is_download" -eq 0 ]; then
		# write anime and episode number and save to temporary history
		sed -E "
			s/^${selection_id}\t[0-9]+/${selection_id}\t$((episode+1))/
		" "$logfile" > "${logfile}.new"

		kill "$PID" >/dev/null 2>&1

		if [ -z "$video_url" ]; then
			die "Video URL not found"
		fi

		

		case $player_fn in
			vlc)
				if [ "$auto_play" -eq 0 ]; then
					nohup "$player_fn" "$player_arguments" --http-referrer="$dpage_link" "$video_url" > /dev/null 2>&1 &
				else
					inf "Currently playing $selection_id episode" "$episode/$last_ep_number"
					"$player_fn" "$player_arguments" --play-and-exit --http-referrer="$dpage_link" "$video_url" > /dev/null 2>&1
					sleep 2
				fi
				;;
			*)
				if [ "$auto_play" -eq 0 ]; then
					echo "$player_arguments" | xargs nohup "$player_fn" --referrer="$dpage_link" "$video_url" --title="ani-cli: $anime_id ep $episode" > /dev/null 2>&1 &
				else
					inf "Currently playing $selection_id episode" "$episode/$last_ep_number"
					echo "$player_arguments" | xargs "$player_fn" --referrer="$dpage_link" "$video_url" --title="ani-cli: $anime_id ep $episode" > /dev/null 2>&1
					sleep 2
				fi
				;;
		esac
		PID=$!

		mv "${logfile}.new" "$logfile"
	else
		mkdir -p "$download_dir"
		inf "Downloading episode $episode ...\n"
		# add 0 padding to the episode name
		episode=$(printf "%03d" "$episode")
		{
		    #uncomment this below line if you are getting low download speeds, and comment next one after below line
			#aria2c -x 16 -s 16 --referer="$dpage_link" "$video_url" --dir="$download_dir" -o "${anime_id}-${episode}.mp4" --download-result=hide &&
			if [ -z "$player_arguments" ]; then
				if aria2c --referer="$dpage_link" "$video_url" --dir="$download_dir" -o "${anime_id}-${episode}.mp4" --download-result=hide ;then
					inf "Downloaded episode: $episode"
				else
					err "Download failed episode: $episode , please retry or check your internet connection"
				fi
			else
				if aria2c "$player_arguments" --referer="$dpage_link" "$video_url" --dir="$download_dir" -o "${anime_id}-${episode}.mp4" --download-result=hide ;then
					inf "Downloaded episode: $episode"
				else
					err "Download failed episode: $episode , please retry or check your internet connection"
				fi
			fi

		}
	fi
}

############
# Start Up #
############

# to clear the colors when exited using SIGINT
trap 'printf "\033[0m"; exit 1' INT HUP

# create history file if none found
[ -f "$logfile" ] || : > "$logfile"

# default options
player_fn="mpv" #video player needs to be able to play urls
is_download=0
half_ep=0
quality=best
scrape=query
download_dir="."
choice=""

while getopts 'viq:dp:chDUVe:a:' OPT; do
	case $OPT in
		h)
			help_text
			exit 0
			;;
		d)
			is_download=1
			;;
		a)
			ep_choice_to_start=$OPTARG
			;;
		D)
			: > "$logfile"
			exit 0
			;;
		p)
			is_download=1
			download_dir=$OPTARG
			;;
		e)
			player_arguments=$OPTARG
			;;
		i)
			player_fn="iina"
			;;
		q)
			quality=$OPTARG
			;;
		c)
			scrape=history
			;;
		v)
			player_fn="vlc"
			;;
		U)
			update_script
			exit 0
			;;
		V)
			version_text
			exit 0
			;;
		*)
			help_text
			exit 1
			;;
	esac
done

# check for main dependencies
dep_ch "curl" "sed" "grep" "git" "openssl"

# check for optional dependencies
if [ "$is_download" -eq 0 ]; then
	dep_ch "$player_fn"
else
	dep_ch "aria2c"
fi

shift $((OPTIND - 1))
# gogoanime likes to change domains but keep the olds as redirects
base_url=$(curl -s -L -o /dev/null -w "%{url_effective}\n" https://gogoanime.cm)
case $scrape in
	query)
		if [ -z "$*" ]; then
			prompt "Search Anime"
			query="$REPLY $REPLY2"
		else
			[ -n "$ep_choice_to_start" ] && select_first=1
			query=$*
		fi
		search_results=$(search_anime "$query")
		[ -z "$search_results" ] && die "No search results found"
		anime_selection "$search_results"
		episode_selection
		;;
	history)
		search_results=$(sed -n -E 's/\t[0-9]*//p' "$logfile")
		[ -z "$search_results" ] && die "History is empty"
		search_results=$(search_for_unwatched "$search_results")
		anime_selection "$search_results"
		first_ep_number="0"
		result=$(get_dpage_link "$anime_id" "$first_ep_number")
		if [ -n "$result" ]; then
			true
		else
			first_ep_number="1"
		fi
		ep_choice_start=$(sed -n -E "s/${selection_id}\t//p" "$logfile")
		;;
	*)
		die "Unexpected Scrape type"
esac

	check_input
	append_history
	open_selection

########
# Loop #
########

while :; do
if [ -z "$select_first" ]; then
	if [ "$auto_play" -eq 0 ]; then
		inf "Currently playing $selection_id episode" "$episode/$last_ep_number"
	else
		auto_play=0
	fi
	if [ "$episode" -ne "$last_ep_number" ]; then
		menu_line_alternate "next episode" "n"
	fi
	if [ "$episode" -ne "$first_ep_number" ]; then
		menu_line_alternate "previous episode" "p"
	fi
	if [ "$last_ep_number" -ne "$first_ep_number" ]; then
		menu_line_alternate "select episode" "s"
	fi
	menu_line_alternate "replay current episode" "r"
	menu_line_alternate "search for another anime" "a"
	menu_line_alternate "search history" "h"
	menu_line_strong "exit" "q"
	prompt "Enter choice"
	# process user choice
	choice="$REPLY"
	case $choice in
		n)
			ep_choice_start=$((episode + 1))
			ep_choice_end=""
			;;
		p)
			ep_choice_start=$((episode - 1))
			ep_choice_end=""
			;;

		s)	episode_selection
			;;

		r)
			ep_choice_start=$((episode))
			ep_choice_end=""
			;;
		a)
			tput reset
			prompt "Search Anime"
			query="$REPLY $REPLY2"
			search_results=$(search_anime "$query")
			[ -z "$search_results" ] && die "No search results found"
			anime_selection "$search_results"
			episode_selection
			;;
		h)
			tput reset
			search_results=$(sed -n -E 's/\t[0-9]*//p' "$logfile")
			[ -z "$search_results" ] && die "History is empty"
			search_results=$(search_for_unwatched "$search_results")
			anime_selection "$search_results"
			ep_choice_start=$(sed -n -E "s/${selection_id}\t//p" "$logfile")
			;;

		N)
			ep_choice_start=$((episode + 1))
			;;
		q)
			break;;

		*)
			tput reset
			err "invalid choice"
			continue
			;;
	esac
	check_input
	append_history
	open_selection
	
else
	wait $!
	exit
fi
done
