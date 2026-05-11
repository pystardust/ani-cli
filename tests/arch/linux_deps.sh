#!/bin/sh
# Architectural invariant: the Linux packages (.deb + AppImage) must
# ship the small POSIX-side ani-cli dependencies bundled in the
# package, and the .deb must declare `Recommends: ffmpeg` so apt
# pulls the heavy distro build automatically.
#
# Without this, ani-cli's `dep_ch fzf` aborts the script at startup
# on a clean Ubuntu / Fedora desktop, silently bricking playback —
# even before the download path needs ffmpeg/aria2c. The Windows
# installer has the same shape (`fetch-windows-deps.mjs` + NSIS
# bundle); this test keeps the Linux side in lockstep so a future
# refactor can't quietly drop one side.
#
# Specifically:
#   - `gui/electron/scripts/fetch-linux-deps.mjs` must exist as the
#     fetch driver (mirror of fetch-windows-deps.mjs).
#   - `gui/electron/package.json` must list `build-resources/linux/bin`
#     under `build.linux.extraResources` so electron-builder copies
#     the staged binaries into both AppImage and .deb payloads.
#   - `build.deb.recommends` must include "ffmpeg".
#   - `dist` / `dist:release` scripts must chain `fetch:linux-deps`
#     so any invocation path (package, dist, e2e) gets the bin
#     dir populated before electron-builder runs.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui/electron ]; then
    printf 'arch/linux_deps: gui/electron does not exist yet — skipping\n'
    exit 0
fi

failed=0
PKG=gui/electron/package.json

# 1. The fetch driver script must exist alongside its Windows sibling.
if [ ! -f gui/electron/scripts/fetch-linux-deps.mjs ]; then
    printf 'arch/linux_deps FAIL: missing gui/electron/scripts/fetch-linux-deps.mjs\n' >&2
    failed=1
fi

# 2. `build.linux.extraResources` must include the build-resources/linux/bin entry.
#    Use a literal substring check: jq isn't a hard dep of the arch tests.
if ! grep -q '"from": *"build-resources/linux/bin"' "$PKG"; then
    printf 'arch/linux_deps FAIL: %s missing build.linux.extraResources entry for build-resources/linux/bin\n' "$PKG" >&2
    failed=1
fi

# 3. `build.deb.recommends` must include "ffmpeg" so `apt install ./...deb`
#    auto-pulls the distro build. Match on the array entry to avoid
#    false-positives on freeform mentions.
if ! grep -Pzo '"recommends"\s*:\s*\[[^]]*"ffmpeg"' "$PKG" >/dev/null 2>&1; then
    printf 'arch/linux_deps FAIL: %s missing "ffmpeg" in build.deb.recommends\n' "$PKG" >&2
    failed=1
fi

# 4. `dist` / `dist:release` scripts must chain `fetch:linux-deps` so
#    callers that invoke dist directly (e2e workflow) still get the bin
#    populated. The fetch-by-package pattern alone leaves a gap.
for s in dist dist:release; do
    line=$(grep -E "\"$s\"" "$PKG" || true)
    case "$line" in
        *fetch:linux-deps*) ;;
        *)
            printf 'arch/linux_deps FAIL: %s "%s" script does not chain fetch:linux-deps\n' "$PKG" "$s" >&2
            failed=1
            ;;
    esac
done

if [ "$failed" -ne 0 ]; then
    exit 1
fi
printf 'arch/linux_deps: OK\n'
