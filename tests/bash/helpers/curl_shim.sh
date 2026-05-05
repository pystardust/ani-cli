#!/bin/sh
#
# Mock `curl` shim placed on PATH for acceptance tests. Pattern-matches the
# combined arguments and returns the appropriate fixture from
# $CURL_FIXTURE_DIR. See tests/bash/acceptance/*.bats for callers.
#
# Routing rules (first match wins):
#   - body contains "episodeString" → episode_blob.json (a synthesized
#     allanime API response with a valid encrypted tobeparsed blob)
#   - GET URL contains both "variables=" and "extensions=" → same as above
#   - body contains "showId" → episodes_short.json
#   - body contains '"search"' → search_one_piece.json
#   - GET URL hits "allanime.day/" but not "/api" → embed_simple.json
#     (the wixmp default embed)
#   - otherwise → fail loudly to surface unmocked calls

set -eu

args="$*"
fixtures="${CURL_FIXTURE_DIR:?CURL_FIXTURE_DIR not set}"

case "$args" in
    *episodeString*)
        cat "$fixtures/episode_blob.json"
        ;;
    *variables=*extensions=*)
        cat "$fixtures/episode_blob.json"
        ;;
    *showId*)
        cat "$fixtures/episodes_short.json"
        ;;
    *'"search"'*)
        cat "$fixtures/search_one_piece.json"
        ;;
    *allanime.day/*)
        # Embed page fetch (any provider path) → return the same simple
        # wixmp embed for every fetch. Provider branches that cannot decode
        # this response just emit nothing, which is the point — only one
        # provider needs to return a usable link for select_quality to pick.
        cat "$fixtures/embed_simple.json"
        ;;
    *)
        printf 'curl shim: no fixture for: %s\n' "$args" >&2
        exit 1
        ;;
esac
