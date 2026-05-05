# Process-stub helper for tests under tests/bash/subprocess/.
#
# Replaces named external commands (mpv, vlc, nohup, am, flatpak, etc.) with
# bash functions that log their argv to a file instead of executing. Tests
# assert on the log to verify ani-cli invoked the right command with the
# right arguments without actually launching anything.
#
# Usage in a .bats setup() (after source_ani_cli_lib):
#
#   load '../helpers/process_stub'
#   stub_setup
#   stub_command nohup mpv vlc flatpak am catt logger
#
# Then exercise the SUT and:
#
#   stub_assert_called nohup '.*mpv.*--force-media-title.*'
#
# All stubs return 0 by default. If a test needs a stub to fail, override
# its function definition after stub_command is called.

# shellcheck shell=bash

# stub_setup — initialize stub state. Creates a calls log under BATS_TEST_TMPDIR.
stub_setup() {
    export STUB_DIR="${BATS_TEST_TMPDIR:?BATS_TEST_TMPDIR not set}/stubs"
    export STUB_CALLS="$STUB_DIR/calls.log"
    mkdir -p "$STUB_DIR"
    : >"$STUB_CALLS"
}

# stub_command CMD [CMD ...] — register one or more named commands as stubs.
# Each stub appends a single line to STUB_CALLS:
#     <cmd> <arg1> <arg2> ...
# Args are space-separated; arguments containing whitespace are not quoted
# in the log (test assertions use grep / [[ =~ ]] pattern matching, not
# byte-for-byte equality, so this is fine).
stub_command() {
    local cmd
    for cmd in "$@"; do
        eval "
$cmd() {
    printf '%s' '$cmd' >>\"\$STUB_CALLS\"
    for a in \"\$@\"; do
        printf ' %s' \"\$a\" >>\"\$STUB_CALLS\"
    done
    printf '\n' >>\"\$STUB_CALLS\"
    return 0
}
"
        export -f "$cmd"
    done
}

# stub_assert_called CMD PATTERN — fail the test unless one logged invocation
# of CMD matches PATTERN (basic-regex, anchored at start). Prints the full
# call log on failure for diagnosis.
stub_assert_called() {
    local cmd="$1" pattern="$2"
    if ! grep -E "^${cmd} .*${pattern}" "$STUB_CALLS" >/dev/null; then
        printf 'stub_assert_called failed:\n  cmd=%s\n  pattern=%s\nfull log:\n' "$cmd" "$pattern" >&2
        cat "$STUB_CALLS" >&2
        return 1
    fi
}

# stub_assert_not_called CMD — fail the test if any invocation of CMD was logged.
stub_assert_not_called() {
    local cmd="$1"
    if grep -E "^${cmd} " "$STUB_CALLS" >/dev/null; then
        printf 'stub_assert_not_called failed: %s was called\nfull log:\n' "$cmd" >&2
        cat "$STUB_CALLS" >&2
        return 1
    fi
}

# stub_calls — print the full calls log (for debugging in failing tests).
stub_calls() {
    cat "$STUB_CALLS"
}
