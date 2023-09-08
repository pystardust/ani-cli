#!/bin/sh
# SCRAPING

# extract the video links from reponse of embed urls, extract mp4 links form m3u8 lists
get_links() {
    episode_link="$(curl -e "$allanime_base" -s --cipher "AES256-SHA256" "https://embed.ssbcontent.site$*" -A "$agent" | sed 's|},{|\n|g' | sed -nE 's|.*link":"([^"]*)".*"resolutionStr":"([^"]*)".*|\2 >\1|p;s|.*hls","url":"([^"]*)".*"hardsub_lang":"en-US".*|\1|p')"
    case "$episode_link" in
        *repackager.wixmp.com*)
            extract_link=$(printf "%s" "$episode_link" | cut -d'>' -f2 | sed 's|repackager.wixmp.com/||g;s|\.urlset.*||g')
            for j in $(printf "%s" "$episode_link" | sed -nE 's|.*/,([^/]*),/mp4.*|\1|p' | sed 's|,|\n|g'); do
                printf "%s >%s\n" "$j" "$extract_link" | sed "s|,[^/]*|${j}|g"
            done | sort -nr
            ;;
        *vipanicdn* | *anifastcdn*)
            if printf "%s" "$episode_link" | head -1 | grep -q "original.m3u"; then
                printf "%s" "$episode_link"
            else
                extract_link=$(printf "%s" "$episode_link" | head -1 | cut -d'>' -f2)
                relative_link=$(printf "%s" "$extract_link" | sed 's|[^/]*$||')
                curl -e "$allanime_base" -s --cipher "AES256-SHA256" "$extract_link" -A "$agent" | sed 's|^#.*x||g; s|,.*|p|g; /^#/d; $!N; s|\n| >|' | sed "s|>|>${relative_link}|g" | sort -nr
            fi
            ;;
        *) [ -n "$episode_link" ] && printf "%s\n" "$episode_link" ;;
    esac
    [ -z "$ANI_CLI_NON_INTERACTIVE" ] && printf "\033[1;32m%s\033[0m Links Fetched\n" "$provider_name" 1>&2
}

# innitialises provider_name and provider_id. First argument is the provider name, 2nd is the regex that matches that provider's link
provider_init() {
    provider_name=$1
    provider_id=$(printf "%s" "$resp" | sed -n "$2" | head -1 | cut -d':' -f2)
}

decrypt_allanime() {
    printf "%s" "$-" | grep -q 'x' && set +x
    for hex in $(printf '%s' "$1" | sed 's/../&\n/g'); do
        dec=$(printf '%d' "0x$hex")
        xor=$((dec ^ 56))
        oct=$(printf "%03o" "$xor")
        #shellcheck disable=SC2059
        printf "\\$oct"
    done
    printf "%s" "$-" | grep -q 'x' || set -x
}

# generates links based on given provider
generate_link() {
    case $1 in
        1) provider_init "wixmp" "/Default :/p" ;;     # wixmp(default)(m3u8)(multi) -> (mp4)(multi)
        2) provider_init "dropbox" "/Sak :/p" ;;       # dropbox(mp4)(single)
        3) provider_init "wetransfer" "/Kir :/p" ;;    # wetransfer(mp4)(single)
        4) provider_init "sharepoint" "/S-mp4 :/p" ;;  # sharepoint(mp4)(single)
        *) provider_init "gogoanime" "/Luf-mp4 :/p" ;; # gogoanime(m3u8)(multi)
    esac
    provider_id="$(decrypt_allanime "$provider_id" | sed "s/\/clock/\/clock\.json/")"
    [ -n "$provider_id" ] && get_links "$provider_id"
}

select_quality() {
    case "$1" in
        best) result=$(printf "%s" "$links" | head -n1) ;;
        worst) result=$(printf "%s" "$links" | grep -E '^[0-9]{3,4}' | tail -n1) ;;
        *) result=$(printf "%s" "$links" | grep -m 1 "$1") ;;
    esac
    [ -z "$result" ] && printf "Specified quality not found, defaulting to best\n" 1>&2 && result=$(printf "%s" "$links" | head -n1)
    printf "%s" "$result" | cut -d'>' -f2
}

# gets embed urls, collects direct links into provider files, selects one with desired quality into $episode
get_episode_url() {
    # get the embed urls of the selected episode
    episode_embed_gql="query (\$showId: String!, \$translationType: VaildTranslationTypeEnumType!, \$episodeString: String!) {    episode(        showId: \$showId        translationType: \$translationType        episodeString: \$episodeString    ) {        episodeString sourceUrls    }}"

    resp=$(curl -e "$allanime_base" -s --cipher "AES256-SHA256" -G "${allanime_api}/api" --data-urlencode "variables={\"showId\":\"$id\",\"translationType\":\"$mode\",\"episodeString\":\"$ep_no\"}" --data-urlencode "query=$episode_embed_gql" -A "$agent" | tr '{}' '\n' | sed 's|\\u002F|\/|g;s|\\||g' | sed -nE 's|.*sourceUrl":"--([^"]*)".*sourceName":"([^"]*)".*|\2 :\1|p')
    # generate links into sequential files
    cache_dir="$(mktemp -d)"
    providers="1 2 3 4 5"
    for provider in $providers; do
        generate_link "$provider" >"$cache_dir"/"$provider" &
    done
    wait
    # select the link with matching quality
    links=$(cat "$cache_dir"/* | sed 's|^Mp4-||g;/http/!d' | sort -g -r -s)
    rm -r "$cache_dir"
    episode=$(select_quality "$quality")
    [ -z "$episode" ] && die "Episode not released!"
}

# search the query and give results
search_anime() {
    search_gql="query(        \$search: SearchInput        \$limit: Int        \$page: Int        \$translationType: VaildTranslationTypeEnumType        \$countryOrigin: VaildCountryOriginEnumType    ) {    shows(        search: \$search        limit: \$limit        page: \$page        translationType: \$translationType        countryOrigin: \$countryOrigin    ) {        edges {            _id name availableEpisodes __typename       }    }}"

    curl -e "$allanime_base" -s --cipher "AES256-SHA256" -G "${allanime_api}/api" --data-urlencode "variables={\"search\":{\"allowAdult\":false,\"allowUnknown\":false,\"query\":\"$1\"},\"limit\":40,\"page\":1,\"translationType\":\"$mode\",\"countryOrigin\":\"ALL\"}" --data-urlencode "query=$search_gql" -A "$agent" | sed 's|Show|\n|g' | sed -nE "s|.*_id\":\"([^\"]*)\",\"name\":\"([^\"]*)\".*${mode}\":([1-9][^,]*).*|\1\t\2 (\3 episodes)|p"
}

# get the episodes list of the selected anime
episodes_list() {
    episodes_list_gql="query (\$showId: String!) {    show(        _id: \$showId    ) {        _id availableEpisodesDetail    }}"

    curl -e "$allanime_base" -s --cipher AES256-SHA256 -G "${allanime_api}/api" --data-urlencode "variables={\"showId\":\"$*\"}" --data-urlencode "query=$episodes_list_gql" -A "$agent" | sed -nE "s|.*$mode\":\[([0-9.\",]*)\].*|\1|p" | sed 's|,|\n|g; s|"||g' | sort -n -k 1
}
