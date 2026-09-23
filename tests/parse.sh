#!/bin/sh
# Offline tests for the html parsing of ani-cli. They feed saved pages to the
# parsing functions instead of the network. Run: sh tests/parse.sh

cd "$(dirname "$0")/.." || exit 1

# load the functions only: everything before the "# MAIN" marker, which starts the setup and main part
grep -q '^# MAIN$' ani-cli || {
    printf 'the "# MAIN" marker is missing in ani-cli, refusing to load it\n'
    exit 1
}
# shellcheck disable=SC2312
eval "$(sed -n '1,/^# MAIN$/p' ani-cli)"

# no network: every request returns the fixture named in $fixture
# shellcheck disable=SC2317,SC2329
hianime_curl() {
    cat "tests/fixtures/$fixture"
}
# shellcheck disable=SC2034
base_api="https://hianime.at"
# shellcheck disable=SC2034
search_api="${base_api}/search?keyword=%s"
# shellcheck disable=SC2034
curl_exe="curl"

failed=0
esc="$(printf '\033')"

# $1 = name, $2 = expected file, stdin = actual output
check() {
    if _diff="$(diff -u "$2" - 2>&1)"; then
        printf 'ok   %s\n' "$1"
    else
        printf 'FAIL %s\n%s\n' "$1" "$_diff"
        failed=1
    fi
}

fixture="search.html"
# the details column is padded with spaces, drop them so the expected file has no trailing whitespace
hianime_search "tests" | sed 's/ *$//' | check "search results carry id, title and details" tests/expected/search.tsv

fixture="detail.html"
(hianime_info "frieren-beyond-journeys-end-481") | sed "s/${esc}\[[0-9;]*m//g" | check "details of an anime" tests/expected/info.txt

if (hianime_info "bad/id") 2>&1 | grep -q "Invalid anime id"; then
    printf 'ok   %s\n' "invalid ids are refused"
else
    printf 'FAIL %s\n' "invalid ids are refused"
    failed=1
fi

# the search prompt reloads its list while typing only with an fzf that can do it
# shellcheck disable=SC2317,SC2329
fzf() {
    printf '%s (test)\n' "$fzf_version"
}
for fzf_version in 0.9.0 0.19.1 0.24.0; do
    if fzf_reloads; then
        printf 'FAIL fzf %s must not use live search\n' "$fzf_version"
        failed=1
    else
        printf 'ok   fzf %s keeps the plain prompt\n' "$fzf_version"
    fi
done
for fzf_version in 0.25.0 0.44.1 1.0.0; do
    if fzf_reloads; then
        printf 'ok   fzf %s searches while typing\n' "$fzf_version"
    else
        printf 'FAIL fzf %s must use live search\n' "$fzf_version"
        failed=1
    fi
done

exit "$failed"
