#!/usr/bin/env bats
#
# Unit tests for `where_iina` (lines 129-132) and `where_mpv` (lines 134-137).
#
# Contract — where_iina:
#   - If /Applications/IINA.app/Contents/MacOS/iina-cli exists → return that
#     absolute path.
#   - Otherwise → "iina".
#
# Contract — where_mpv:
#   - If `flatpak` exists AND `flatpak info io.mpv.Mpv` succeeds → "flatpak_mpv".
#   - Otherwise → "mpv".
#
# Both functions are platform-conditional; we test the deterministic
# fallbacks via mocking `command -v` and `flatpak`.

load '../helpers/loader'

@test "where_iina: returns 'iina' when the IINA.app path is absent" {
    # Linux CI never has /Applications/IINA.app/, so this is the natural case.
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; where_iina'
    [ "$status" -eq 0 ]
    [ "$output" = "iina" ]
}

@test "where_mpv: returns 'mpv' when flatpak is not installed" {
    # Mock command -v to return failure for flatpak.
    run bash -c '
        __ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null
        command() {
            if [ "$1" = "-v" ] && [ "$2" = "flatpak" ]; then return 1; fi
            builtin command "$@"
        }
        where_mpv
    '
    [ "$status" -eq 0 ]
    [ "$output" = "mpv" ]
}

@test "where_mpv: returns 'flatpak_mpv' when flatpak info reports the mpv flatpak" {
    # Mock both command -v flatpak (success) and flatpak (return success on `info`).
    run bash -c '
        __ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null
        command() {
            if [ "$1" = "-v" ] && [ "$2" = "flatpak" ]; then return 0; fi
            builtin command "$@"
        }
        flatpak() { return 0; }
        where_mpv
    '
    [ "$status" -eq 0 ]
    [ "$output" = "flatpak_mpv" ]
}

@test "where_mpv: returns 'mpv' when flatpak exists but mpv flatpak is not installed" {
    run bash -c '
        __ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null
        command() {
            if [ "$1" = "-v" ] && [ "$2" = "flatpak" ]; then return 0; fi
            builtin command "$@"
        }
        flatpak() { return 1; }   # info call fails
        where_mpv
    '
    [ "$status" -eq 0 ]
    [ "$output" = "mpv" ]
}
