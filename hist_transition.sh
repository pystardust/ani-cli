#!/bin/sh

histfile="${XDG_CACHE_HOME:-$HOME/.cache}/ani-hsts"
base_url="https://gogoplay4.com"

die () {
	err "$*"
	exit 1
}

# display an error message to stderr (in red)
err () {
	printf "\033[1;31m%s\033[0m\n" "$*" >&2
}

inf () {
	# display an informational message (first argument in green, second in magenta)
	printf "\033[1;32m%s \033[1;35m%s\033[0m\n" "$1" "$2"
}

search_anime () {
	search=$(printf '%s' "$1" | tr ' ' '-' )
	curl -s "$base_url/search.html" -G -d "keyword=$search" |
		sed -nE 's_^[[:space:]]*<a href="/videos/([^"]*)">_\1_p'
}

extended_search () {
	indexing_url=$(curl -s -L -o /dev/null -w "%{url_effective}\n" https://gogoanime.cm)
	search=$(printf '%s' "$1" | tr ' ' '-' )
	curl -s "$indexing_url//search.html" -G -d "keyword=$search" |
		sed -n -E 's_^[[:space:]]*<a href="/category/([^"]*)" title="([^"]*)".*_\1_p'
}

update_entry () {
	query=$(printf "%s" "$anime_id" | sed 's/[0-9]*.$//' | sed 's/\t//')
	history_ep_number=$(printf "%s" "$anime_id" | sed "s/${query}\t//g")
	search_results=$(search_anime "$query")
	if [ -z "$search_results" ]; then
		extended_search_results=$(extended_search "$query")
		if [ -n "$extended_search_results" ]; then
			extended_search_results=$(printf '%s' "$extended_search_results" | head -n 1)
			search_results=$(search_anime "$extended_search_results")
		else
			err "Can't find ${query}, you'll have to add this to your history manually (by playing it). You have episode ${history_ep_number} coming up."
			return
		fi
	fi
	printf "%s\n" "$search_results" | head -n 1 | sed "s/[0-9]*.$/${history_ep_number}/" | sed 's/\t//' >> "${histfile}.new"
}

	inf "Transferring history..."
	search_results=$(cat "$histfile")
	[ -z "$search_results" ] && die "History is empty"
	while read -r anime_id; do
		update_entry &
	done <<-EOF
	$search_results
	EOF
	wait
	mv "${histfile}.new" "$histfile"