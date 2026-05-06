#!/bin/sh
# .deb postinst hook — runs as root after dpkg lays down /opt/ani-gui/.
#
# Electron's chrome-sandbox helper has to be owned by root with mode
# 4755 (setuid) for the renderer sandbox to attach. electron-builder's
# stock packaging stopped setting this in newer releases (the kernel
# trend is toward unprivileged user namespaces instead) — but Ubuntu
# 24.04+ restricts those via AppArmor, so the fallback path is the
# classic SUID sandbox. Without this, launching from the menu fails
# with `setuid_sandbox_host.cc(163) … not configured correctly`.

set -e

SANDBOX=/opt/ani-gui/chrome-sandbox
if [ -e "$SANDBOX" ]; then
    chown root:root "$SANDBOX"
    chmod 4755 "$SANDBOX"
fi

# /usr/local/bin symlink so users can launch from a terminal with
# `ani-gui`. electron-builder's stock packaging puts the launcher at
# /opt/ani-gui/ani-gui without exposing it on PATH; we add the link
# explicitly. /usr/local/bin (rather than /usr/bin) because dpkg-shipped
# binaries live in /usr/bin and shouldn't be touched by user packages.
LAUNCHER=/opt/ani-gui/ani-gui
LINK=/usr/local/bin/ani-gui
if [ -x "$LAUNCHER" ]; then
    mkdir -p /usr/local/bin
    ln -sf "$LAUNCHER" "$LINK"
fi

# Bundled ani-cli script needs the executable bit — extraResources copies
# it but dpkg's tar extraction doesn't always preserve the +x bit on
# files that lack it in the source tree.
ANI_CLI=/opt/ani-gui/resources/ani-cli
if [ -e "$ANI_CLI" ]; then
    chmod 0755 "$ANI_CLI"
fi

exit 0
