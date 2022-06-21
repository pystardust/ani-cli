#!/bin/sh

histfile="${XDG_CACHE_HOME:-$HOME/.cache}/ani-hsts"
base_url="https://animixplay.to"

die () {
	err "$*"
	exit 1
}

# display an error message to stderr (in red)
err () {
	printf "\033[1;31m%s\033[0m\n" "$*" >&2
}

inf () {
	# display an informational message
	printf "\033[1;35m%s\033[0m\n" "$*"
}

search_anime () {
	search=$(printf '%s' "$1" | tr '-' '+' )
	curl -A 'uwu' -s -X POST "$base_url/api/search/v1" -d "q2=$search" |
		sed -e 's_</li>_\n_g' -e 's/\\//g' | sed -nE 's_.*a href="/v1/([^"]*)".*_\1_p' | head -1
}

update_entry () {
	query=$(printf "%s" "$anime_id" | sed 's/-episode.*//')
	history_ep_number=$(printf "%s" "$anime_id" | sed "s/${query}-episode-//g")
	search_result=$(search_anime "$query")
	if [ -z "$search_result" ]; then
		err "Can't find ${query}, you'll have to add this to your history manually (by playing it). You have episode ${history_ep_number} coming up."
		return 0
	fi
	printf "\033[1;32mFound Match for %s >> %s\n" "$query" "$search_result"
	printf "%s\t%s\n" "$search_result" "$history_ep_number" >> "${histfile}.new"
}

	inf "Transferring history..."
	history_results=$(cat "$histfile")
	[ -z "$history_results" ] && die "History is empty"
	while read -r anime_id; do
		update_entry &
	done <<-EOF
	$history_results
	EOF
	wait
	inf "Done.."
	mv "${histfile}.new" "$histfile"
