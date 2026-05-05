#!/usr/bin/env bash
#
# Build a synthetic episode_blob.json fixture for the acceptance tests. The
# response shape mirrors what allanime's API returns when its GraphQL
# endpoint serves a "tobeparsed" blob: a base64-encoded AES-256-CTR
# ciphertext that decodes to one or more {"sourceUrl","sourceName"} JSON
# objects.
#
# Usage:
#   blob_builder.sh <output_path>
#
# Hardcoded plaintext:
#   {"sourceUrl":"--174c5d4b4c","sourceName":"Default","priority":1}
#
# 174c5d4b4c decodes (via the substitution table inside provider_init) to
# the path "/test", which the embed-page fetch step reaches at
# https://allanime.day/test. The curl shim returns embed_simple.json for
# any allanime.day/* GET, so the wixmp-default branch produces a usable
# link and the other 4 providers fail silently.

set -eu

out="${1:?usage: blob_builder.sh <output_path>}"

# Same key derivation as ani-cli line 466.
allanime_key="$(printf '%s' 'Xot36i3lK3:v1' | openssl dgst -sha256 -binary | od -A n -t x1 | tr -d ' \n')"

iv_hex='000102030405060708090a0b'
ctr_hex="${iv_hex}00000002"
plaintext='{"sourceUrl":"--174c5d4b4c","sourceName":"Default","priority":1}'

tmp=$(mktemp)
{
    # 1-byte prefix
    printf '\x00'
    # 12 bytes of IV
    printf '%s' "$iv_hex" | xxd -r -p
    # ciphertext
    printf '%s' "$plaintext" |
        openssl enc -aes-256-ctr -K "$allanime_key" -iv "$ctr_hex" -nosalt -nopad
    # 16 bytes of trailing padding
    dd if=/dev/zero bs=1 count=16 2>/dev/null
} >"$tmp"

blob_b64=$(base64 -w0 <"$tmp")
rm -f "$tmp"

cat >"$out" <<EOF
{"data":{"episode":{"episodeString":"1","sourceUrls":[],"tobeparsed":"${blob_b64}","__typename":"Episode"}}}
EOF
