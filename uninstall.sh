#!/bin/sh
set -e

INSTALL_DIR="${ANI_CLI_INSTALL_DIR:-/usr/local/bin}"

printf "\033[1;34mUninstalling ani-cli...\033[0m\n"

_removed=0

for _file in ani-cli aesgcm aesgcm.c discord-rpc discord-rpc.c; do
    _path="${INSTALL_DIR}/${_file}"
    if [ -f "$_path" ] || [ -L "$_path" ]; then
        rm -f "$_path"
        printf "  Removed %s\n" "$_path"
        _removed=1
    fi
done

if [ "$_removed" -eq 0 ]; then
    printf "\033[1;31mani-cli not found in %s\033[0m\n" "$INSTALL_DIR"
    exit 1
fi

printf "\033[1;32mani-cli uninstalled successfully!\033[0m\n"
