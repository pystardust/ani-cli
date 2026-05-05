#!/usr/bin/env bats
#
# Unit tests for ani-cli's `decode_tobeparsed` (lines 216-226).
#
# Contract:
#   - $1 = base64-encoded blob.
#   - Blob layout (after base64 decode):
#       byte 0           : prefix (skipped)
#       bytes 1..12      : IV (12 bytes)
#       bytes 13..N-16   : ciphertext
#       bytes N-15..N    : trailing 16 bytes (skipped)
#   - Cipher: AES-256-CTR with $allanime_key. Counter is IV (12 bytes) +
#     fixed counter "00000002" (4 bytes). Total IV passed to openssl: 16 bytes.
#   - Plaintext is JSON like:
#       {"sourceUrl":"--<encoded>","sourceName":"<provider>",...}{"sourceUrl":...}
#   - Output: one "<sourceName> :<encoded>" line per object.
#
# Tests round-trip: construct a plaintext, encrypt with the same key/IV
# scheme, wrap in the expected blob layout, and verify decode_tobeparsed
# extracts the expected pairs.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
}

# Encrypt a plaintext using the same scheme decode_tobeparsed expects.
# Args: $1=plaintext, $2=iv_hex (24 hex chars = 12 bytes).
# Echoes the base64-encoded blob to stdout.
encrypt_blob() {
    plaintext="$1"
    iv_hex="$2"
    ctr_hex="${iv_hex}00000002"

    tmp_blob=$(mktemp)
    {
        # 1-byte prefix (the script skips it)
        printf '\x00'
        # 12 bytes of IV
        printf '%s' "$iv_hex" | xxd -r -p
        # ciphertext
        printf '%s' "$plaintext" | \
            openssl enc -aes-256-ctr -K "$allanime_key" -iv "$ctr_hex" -nosalt -nopad
        # 16-byte trailing padding (the script skips it)
        dd if=/dev/zero bs=1 count=16 2>/dev/null
    } >"$tmp_blob"
    base64 -w0 <"$tmp_blob"
    rm -f "$tmp_blob"
}

@test "decode_tobeparsed: round-trips a single sourceName:sourceUrl pair" {
    iv="000102030405060708090a0b"
    plaintext='{"sourceUrl":"--testurl123","sourceName":"wixmp","priority":1}'
    blob=$(encrypt_blob "$plaintext" "$iv")
    output=$(decode_tobeparsed "$blob")
    [ "$output" = "wixmp :testurl123" ]
}

@test "decode_tobeparsed: round-trips multiple pairs (one per line)" {
    iv="0123456789abcdef01234567"
    plaintext='{"sourceUrl":"--abc","sourceName":"wixmp","priority":1}{"sourceUrl":"--def","sourceName":"hianime","priority":2}{"sourceUrl":"--ghi","sourceName":"filemoon","priority":3}'
    blob=$(encrypt_blob "$plaintext" "$iv")
    output=$(decode_tobeparsed "$blob")
    expected="wixmp :abc
hianime :def
filemoon :ghi"
    [ "$output" = "$expected" ]
}

@test "decode_tobeparsed: ignores objects without sourceUrl/sourceName" {
    iv="ffeeddccbbaa99887766554a"
    # Two valid pairs and one decoy object that lacks the expected fields.
    plaintext='{"sourceUrl":"--first","sourceName":"wixmp"}{"unrelated":"value"}{"sourceUrl":"--second","sourceName":"hianime"}'
    blob=$(encrypt_blob "$plaintext" "$iv")
    output=$(decode_tobeparsed "$blob")
    expected="wixmp :first
hianime :second"
    [ "$output" = "$expected" ]
}

@test "decode_tobeparsed: requires the -- prefix on sourceUrl" {
    iv="0badc0ffee0badc0ffee0bad"
    # sourceUrl without -- prefix is skipped per the regex.
    plaintext='{"sourceUrl":"https://no-prefix.example/x","sourceName":"wixmp"}'
    blob=$(encrypt_blob "$plaintext" "$iv")
    output=$(decode_tobeparsed "$blob")
    [ -z "$output" ]
}

@test "decode_tobeparsed: allanime_key is the SHA-256 of Xot36i3lK3:v1" {
    # Sanity check the key derivation. The decode contract is locked to this
    # specific key — if upstream rotates it, every fixture in this suite must
    # be re-recorded.
    expected="$(printf '%s' 'Xot36i3lK3:v1' | openssl dgst -sha256 -binary | od -A n -t x1 | tr -d ' \n')"
    [ "$allanime_key" = "$expected" ]
    [ ${#allanime_key} -eq 64 ]
}
