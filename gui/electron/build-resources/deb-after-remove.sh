#!/bin/sh
# .deb postrm hook — runs as root after dpkg removes /opt/ani-gui/.
#
# Cleans up the /usr/local/bin/ani-gui symlink that the postinst
# created. Only removes if the link still points at our launcher,
# so a user who replaced it manually doesn't lose their override.

set -e

LINK=/usr/local/bin/ani-gui
TARGET=/opt/ani-gui/ani-gui
if [ -L "$LINK" ]; then
    if [ "$(readlink "$LINK")" = "$TARGET" ]; then
        rm -f "$LINK"
    fi
fi

exit 0
