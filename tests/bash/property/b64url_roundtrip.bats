#!/usr/bin/env bats
#
# Property test: b64url_to_hex(hex_to_b64url(x)) == x for random hex strings.
#
# bats has no native shrinking; we emulate property tests with a small
# generator harness. 200 random inputs is enough to catch encoding/padding
# bugs in practice. The harness uses /dev/urandom for entropy, then converts
# to hex, then to base64url (no padding, url-safe alphabet), then asks the
# function to convert it back to hex.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
}

# Generate a random hex string of $1 bytes (so $1 * 2 hex chars).
random_hex() {
    head -c "$1" /dev/urandom | od -A n -t x1 | tr -d ' \n'
}

# Convert hex bytes -> base64url-without-padding.
hex_to_b64url_no_pad() {
    printf '%s' "$1" | xxd -r -p | base64 -w0 | tr '+/' '-_' | tr -d '='
}

@test "b64url_to_hex: round-trip 200 random inputs of varying length (1..64 bytes)" {
    iter=0
    while [ "$iter" -lt 200 ]; do
        len=$(((RANDOM % 64) + 1))
        input_hex=$(random_hex "$len")
        b64url=$(hex_to_b64url_no_pad "$input_hex")
        round_trip=$(b64url_to_hex "$b64url")
        if [ "$input_hex" != "$round_trip" ]; then
            printf 'mismatch at iter %d (len=%d):\n' "$iter" "$len"
            printf '  input_hex:  %s\n' "$input_hex"
            printf '  b64url:     %s\n' "$b64url"
            printf '  round_trip: %s\n' "$round_trip"
            return 1
        fi
        iter=$((iter + 1))
    done
}

@test "b64url_to_hex: round-trip with deterministic edge cases" {
    # Each tuple is "hex base64url-no-pad". Hand-checked.
    cases='
        00 AA
        ff _w
        0001 AAE
        0102 AQI
        010203 AQID
        00112233 ABEiMw
        ffffffff _____w
        00000000 AAAAAA
    '
    while read -r hex b64url; do
        [ -z "$hex" ] && continue
        result=$(b64url_to_hex "$b64url")
        if [ "$result" != "$hex" ]; then
            printf 'b64url_to_hex(%q) = %q, expected %q\n' "$b64url" "$result" "$hex"
            return 1
        fi
    done <<EOF
$cases
EOF
}
