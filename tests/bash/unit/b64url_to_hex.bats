#!/usr/bin/env bats
#
# Unit tests for ani-cli's `b64url_to_hex` (lines 228-237).
#
# Contract:
#   - Takes a base64url-encoded string as $1.
#   - Pads to a multiple of 4 with `=`.
#   - Replaces url-safe chars `-_` with standard base64 `+/`.
#   - Decodes via `base64 -d`.
#   - Outputs the hex representation with no whitespace.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
}

@test "b64url_to_hex: AQID decodes to 010203" {
    # Standard base64: AQID = bytes 0x01 0x02 0x03
    output=$(b64url_to_hex "AQID")
    [ "$output" = "010203" ]
}

@test "b64url_to_hex: empty input yields empty output" {
    output=$(b64url_to_hex "")
    [ -z "$output" ]
}

@test "b64url_to_hex: pads length-2 input with ==" {
    # "AQ" needs 2 chars padding; AQ== decodes to single byte 0x01.
    output=$(b64url_to_hex "AQ")
    [ "$output" = "01" ]
}

@test "b64url_to_hex: pads length-3 input with =" {
    # "AQI" needs 1 char padding; AQI= decodes to 0x01 0x02.
    output=$(b64url_to_hex "AQI")
    [ "$output" = "0102" ]
}

@test "b64url_to_hex: replaces url-safe '_' with '/'" {
    # "_w" → "/w==" → base64 decode → 0xff.
    output=$(b64url_to_hex "_w")
    [ "$output" = "ff" ]
}

@test "b64url_to_hex: replaces url-safe '-' with '+'" {
    # "-w" → "+w==" → base64 decode → 0xfb.
    output=$(b64url_to_hex "-w")
    [ "$output" = "fb" ]
}

@test "b64url_to_hex: handles longer input round-trip" {
    # "Hello" → "SGVsbG8=" base64; without padding it's "SGVsbG8".
    # Hex of "Hello" = 48656c6c6f.
    output=$(b64url_to_hex "SGVsbG8")
    [ "$output" = "48656c6c6f" ]
}
