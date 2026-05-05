#!/usr/bin/env bats
#
# Tests for ani-cli's `update_script` (lines 107-120).
#
# Contract:
#   - curl GETs https://raw.githubusercontent.com/pystardust/ani-cli/master/ani-cli
#   - On curl failure: die "Connection error" (exit 1).
#   - Diff the response against $0. If no diff: "Script is up to date :)".
#   - Otherwise: pipe the diff through `patch "$0" -`.
#       - Patch success: "Script has been updated".
#       - Patch failure: die "Can't update for some reason!".
#   - Always exit 0 on success.
#
# Tests use `run bash -c '...'` so the function's exit doesn't kill the
# bats process. A tmp copy of ani-cli is passed as $0 so `patch "$0"`
# touches the tmp, never the vendored script.

load '../helpers/loader'

@test "update_script: up-to-date prints 'Script is up to date'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        # Mock curl: return the current contents (no diff).
        curl() { cat "$0"; }
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 0 ]
    [[ "$output" == *"up to date"* ]]
}

@test "update_script: with upstream changes calls patch and prints 'updated'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        # Mock curl: return current contents PLUS an extra line so diff is non-empty.
        curl() { cat "$0"; printf "%s\n" "# upstream-added line"; }
        # Mock patch as a successful no-op.
        patch_was_called=0
        patch() { patch_was_called=1; cat >/dev/null; return 0; }
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 0 ]
    [[ "$output" == *"updated"* ]]
}

@test "update_script: failed patch dies with 'Can\\'t update for some reason'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        curl() { cat "$0"; printf "%s\n" "# extra"; }
        patch() { cat >/dev/null; return 1; }   # simulate patch rejecting
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Can't update"* ]]
}

@test "update_script: curl failure dies with 'Connection error'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        curl() { return 1; }   # simulate offline
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Connection error"* ]]
}
