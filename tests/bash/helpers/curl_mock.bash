# Curl mock helper for tests under tests/bash/network/.
#
# Defines a `curl` function in the current shell that intercepts ani-cli's
# outbound HTTP calls and returns canned responses from
# tests/fixtures/allanime/. Enable in a test by `load`ing this file inside
# setup() (after source_ani_cli_lib).
#
# Two modes:
#
#   1. Static — set CURL_MOCK_RESPONSE to an absolute path. Every curl
#      invocation returns that file's contents. Use for tests that make
#      a single HTTP call.
#
#   2. URL-pattern dispatch — set CURL_MOCK_DISPATCH to a function that
#      examines the curl argv array and prints the absolute path of the
#      fixture to use (or an empty string to fail). Use for tests that
#      make multiple HTTP calls with distinguishable URLs/bodies.
#
# In both modes, every invocation is logged: the full argv is appended to
# CURL_MOCK_LOG (a file path the caller sets) so tests can assert on the
# args ani-cli actually constructed.

# shellcheck shell=bash

curl() {
    # Log the call.
    if [ -n "${CURL_MOCK_LOG:-}" ]; then
        printf '%s\n' "$*" >>"$CURL_MOCK_LOG"
    fi

    # URL-pattern dispatch wins over static.
    if [ -n "${CURL_MOCK_DISPATCH:-}" ]; then
        local fixture
        fixture=$("$CURL_MOCK_DISPATCH" "$@")
        if [ -n "$fixture" ] && [ -f "$fixture" ]; then
            cat "$fixture"
            return 0
        fi
        printf 'curl_mock: dispatch returned no fixture for: %s\n' "$*" >&2
        return 1
    fi

    # Static fixture mode.
    if [ -n "${CURL_MOCK_RESPONSE:-}" ] && [ -f "$CURL_MOCK_RESPONSE" ]; then
        cat "$CURL_MOCK_RESPONSE"
        return 0
    fi

    printf 'curl_mock: no CURL_MOCK_RESPONSE or CURL_MOCK_DISPATCH set; got: %s\n' "$*" >&2
    return 1
}

# Reset all mock state. Call between tests if reusing the same shell.
curl_mock_reset() {
    unset CURL_MOCK_RESPONSE CURL_MOCK_DISPATCH
    [ -n "${CURL_MOCK_LOG:-}" ] && [ -f "$CURL_MOCK_LOG" ] && : >"$CURL_MOCK_LOG"
}
