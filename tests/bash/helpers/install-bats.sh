#!/bin/sh
# Install bats-core and its companion plugins at pinned tags into a vendor
# directory inside the repo's gitignored cache. Idempotent — re-running
# checks the existing checkouts and updates only when the pinned tag has
# changed.
#
# CI invokes this once before running the bats suite. Local devs run it once
# after cloning. The resulting tools are available under
# tests/bash/.bats-vendor/ which is gitignored.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../.bats-vendor"

# Pinned versions. Bumping any of these is a manual PR.
BATS_CORE_TAG='v1.11.1'
BATS_ASSERT_TAG='v2.1.0'
BATS_FILE_TAG='v0.4.0'
BATS_MOCK_TAG='v1.2.5'
BATS_SUPPORT_TAG='v0.3.0'

mkdir -p "$VENDOR_DIR"

clone_or_update() {
    name="$1"
    repo="$2"
    tag="$3"
    target="$VENDOR_DIR/$name"

    if [ -d "$target/.git" ]; then
        cur="$(git -C "$target" describe --tags --exact-match 2>/dev/null || echo '')"
        if [ "$cur" = "$tag" ]; then
            printf '  %s already at %s\n' "$name" "$tag"
            return 0
        fi
        printf '  updating %s -> %s\n' "$name" "$tag"
        git -C "$target" fetch --tags --depth 1 origin "$tag" >/dev/null
        git -C "$target" checkout -q "$tag"
    else
        printf '  cloning %s @ %s\n' "$name" "$tag"
        git clone --depth 1 --branch "$tag" "$repo" "$target" >/dev/null 2>&1
    fi
}

printf 'Installing pinned bats toolchain into %s\n' "$VENDOR_DIR"
clone_or_update bats-core    https://github.com/bats-core/bats-core.git    "$BATS_CORE_TAG"
clone_or_update bats-assert  https://github.com/bats-core/bats-assert.git  "$BATS_ASSERT_TAG"
clone_or_update bats-file    https://github.com/bats-core/bats-file.git    "$BATS_FILE_TAG"
clone_or_update bats-support https://github.com/bats-core/bats-support.git "$BATS_SUPPORT_TAG"
clone_or_update bats-mock    https://github.com/jasonkarns/bats-mock.git   "$BATS_MOCK_TAG"

# Make bats discoverable on PATH for the rest of this shell session if sourced.
BATS_BIN="$VENDOR_DIR/bats-core/bin/bats"
if [ -x "$BATS_BIN" ]; then
    printf '\nbats binary: %s\n' "$BATS_BIN"
    "$BATS_BIN" --version
else
    printf 'ERROR: bats binary not found at %s\n' "$BATS_BIN" >&2
    exit 1
fi
