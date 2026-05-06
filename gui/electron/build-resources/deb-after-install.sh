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

exit 0
