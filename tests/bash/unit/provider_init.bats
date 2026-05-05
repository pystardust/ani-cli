#!/usr/bin/env bats
#
# Unit tests for ani-cli's `provider_init` (lines 172-176).
#
# Contract:
#   - $1 = provider name; sets global $provider_name = $1.
#   - $2 = sed regex selector applied to global $resp.
#   - Sets global $provider_id from the matched line, decoded via the
#     character-substitution table inside the function.
#
# The character-substitution table is a custom encoding used by allanime;
# unit tests here exercise the structural contract (provider_name set,
# provider_id behaviour on empty $resp). End-to-end decoding correctness
# is verified later with real captured `resp` fixtures in network tests.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    unset provider_name provider_id
}

@test "provider_init: sets provider_name from first arg" {
    resp=""
    provider_init "wixmp" "/Default :/p"
    [ "$provider_name" = "wixmp" ]
}

@test "provider_init: empty resp yields empty provider_id" {
    resp=""
    provider_init "wixmp" "/Default :/p"
    [ -z "$provider_id" ]
}

@test "provider_init: resp without matching line yields empty provider_id" {
    resp="some unrelated text"
    provider_init "wixmp" "/Default :/p"
    [ -z "$provider_id" ]
}

@test "provider_init: decodes a single mapped pair (79 -> A)" {
    # Per the in-function table: 79 -> A. Input pattern has the colon:
    # cut -d':' -f2 of "Default:79" yields "79", which sed splits into "79"
    # and is mapped to "A". The trailing /clock /clock.json substitution
    # only fires on /clock — irrelevant here.
    resp="Default:79"
    provider_init "wixmp" "/Default:/p"
    [ "$provider_id" = "A" ]
}

@test "provider_init: decodes a mapped sequence (705d54 -> Hel)" {
    # 70->H, 5d->e, 54->l per the table.
    resp="Default:705d54"
    provider_init "wixmp" "/Default:/p"
    [ "$provider_id" = "Hel" ]
}

@test "provider_init: appends .json on /clock paths" {
    # Per the substitution table on line 175 of ani-cli:
    #   '/' -> 17, 'c' -> 5b, 'l' -> 54, 'o' -> 57, 'c' -> 5b, 'k' -> 53
    # (4d maps to 'u', not 'c' — easy to mistake.)
    # The trailing sed `s|/clock|/clock.json|` then appends .json.
    resp="Default:175b54575b53"
    provider_init "wixmp" "/Default:/p"
    [ "$provider_id" = "/clock.json" ]
}
