#!/bin/sh
set -e

REPO="VVAT3R/ani-cli"
BRANCH="fix/aa-crypto-no-botan"
RAW="https://raw.githubusercontent.com/${REPO}/${BRANCH}"

INSTALL_DIR="${ANI_CLI_INSTALL_DIR:-/usr/local/bin}"

printf "\033[1;34mInstalling ani-cli from %s...\033[0m\n" "$REPO"

command -v curl >/dev/null || { printf "\033[1;31mError: curl not found\033[0m\n"; exit 1; }
command -v gcc >/dev/null || { printf "\033[1;31mError: gcc not found (needed to compile aesgcm helper)\033[0m\n"; exit 1; }

_tmpdir="$(mktemp -d)"
trap 'rm -rf "$_tmpdir"' EXIT

printf "Downloading ani-cli...\n"
curl -fsSL "${RAW}/ani-cli" -o "${_tmpdir}/ani-cli" || { printf "\033[1;31mFailed to download ani-cli\033[0m\n"; exit 1; }
chmod +x "${_tmpdir}/ani-cli"

printf "Downloading aesgcm.c...\n"
curl -fsSL "${RAW}/aesgcm.c" -o "${_tmpdir}/aesgcm.c" || { printf "\033[1;31mFailed to download aesgcm.c\033[0m\n"; exit 1; }

printf "Compiling aesgcm helper...\n"
gcc -O2 -o "${_tmpdir}/aesgcm" "${_tmpdir}/aesgcm.c" -lcrypto || { printf "\033[1;31mFailed to compile aesgcm\033[0m\n"; exit 1; }

printf "Compiling discord-rpc helper...\n"
gcc -O2 -o "${_tmpdir}/discord-rpc" "${_tmpdir}/discord-rpc.c" || { printf "\033[1;31mFailed to compile discord-rpc\033[0m\n"; exit 1; }

printf "Installing to %s...\n" "$INSTALL_DIR"
install -d "${INSTALL_DIR}"
install -m 755 "${_tmpdir}/ani-cli" "${INSTALL_DIR}/ani-cli"
install -m 755 "${_tmpdir}/aesgcm" "${INSTALL_DIR}/aesgcm"
install -m 644 "${_tmpdir}/aesgcm.c" "${INSTALL_DIR}/aesgcm.c"
install -m 755 "${_tmpdir}/discord-rpc" "${INSTALL_DIR}/discord-rpc"
install -m 644 "${_tmpdir}/discord-rpc.c" "${INSTALL_DIR}/discord-rpc.c"

printf "\033[1;32mani-cli installed successfully!\033[0m\n"
printf "  Binary:      %s/ani-cli\n" "$INSTALL_DIR"
printf "  aesgcm:     %s/aesgcm\n" "$INSTALL_DIR"
printf "  discord-rpc: %s/discord-rpc\n" "$INSTALL_DIR"
printf "\nRun: ani-cli --help\n"
