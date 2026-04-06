#!/bin/sh
# setup.sh — bootstraps ani-skip + aniskip.lua for ani-cli
# Supports: Linux, macOS, Android-Termux, iOS (iSH), Windows (WSL2/MINGW)
# Usage: run once after cloning ani-cli, then just use ./ani-cli as normal

set -e

C_CYAN="\033[1;36m"
C_GREEN="\033[1;32m"
C_YELLOW="\033[1;33m"
C_RED="\033[1;31m"
C_RESET="\033[0m"

info()    { printf "${C_CYAN}[setup]${C_RESET} %s\n" "$*"; }
success() { printf "${C_GREEN}[setup]${C_RESET} %s\n" "$*"; }
warn()    { printf "${C_YELLOW}[setup]${C_RESET} %s\n" "$*"; }
die()     { printf "${C_RED}[setup]${C_RESET} %s\n" "$*" >&2; exit 1; }

# Detect the platform
UNAME="$(uname -a)"
case "$UNAME" in
    *Darwin*)           PLATFORM="macos"   ;;
    *ndroid*)           PLATFORM="android" ;; # Termux
    *MINGW* | *MSYS*)  PLATFORM="windows" ;; # Git Bash / MINGW
    *WSL2* | *WSL*)    PLATFORM="wsl"     ;;
    *ish*)              PLATFORM="ios"     ;; # iSH
    *)                  PLATFORM="linux"   ;;
esac
info "Detected platform: $PLATFORM"

# Resolve MPV config dir per platform
case "$PLATFORM" in
    macos)
        MPV_CONF_DIR="$HOME/.config/mpv"
        ;;
    android)
        MPV_CONF_DIR="$HOME/.config/mpv"
        ;;
    windows)
        # MINGW / Git Bash
        MPV_CONF_DIR="${APPDATA:-$HOME/AppData/Roaming}/mpv"
        ;;
    wsl)
        # WSL running
        WIN_APPDATA="$(cmd.exe /c "echo %APPDATA%" 2>/dev/null | tr -d '\r')"
        if [ -n "$WIN_APPDATA" ]; then
            MPV_CONF_DIR="$(wslpath "$WIN_APPDATA")/mpv"
        else
            MPV_CONF_DIR="$HOME/.config/mpv"
            warn "Could not resolve Windows APPDATA, falling back to $MPV_CONF_DIR"
        fi
        ;;
    ios)
        # iSH uses VLC skip mpv config
        MPV_CONF_DIR=""
        ;;
    *)
        MPV_CONF_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/mpv"
        ;;
esac

# Resolve PATH install dir per platform
case "$PLATFORM" in
    android)
        BIN_DIR="$PREFIX/bin"
        ;;
    ios)
        BIN_DIR="/usr/local/bin"
        ;;
    windows | wsl)
        BIN_DIR="$HOME/.local/bin"
        ;;
    *)
        # Prefer ~/.local/bin (no sudo), fall back to /usr/local/bin
        BIN_DIR="$HOME/.local/bin"
        ;;
esac

# Ensure BIN_DIR exists and is on PATH
mkdir -p "$BIN_DIR"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;   #already on PATH
    *)
        warn "$BIN_DIR is not on your PATH."
        warn "Add this to your shell rc file (~/.bashrc / ~/.zshrc / ~/.profile):"
        warn "  export PATH=\"\$PATH:$BIN_DIR\""
        # Add it for this session so the rest of setup works
        export PATH="$PATH:$BIN_DIR"
        ;;
esac

# Check for git
command -v git >/dev/null 2>&1 || die "git is required but not found. Please install git first."

# Clone or update ani-skip
ANISKIP_REPO="https://github.com/synacktraa/ani-skip"
ANISKIP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/ani-skip"

if [ -d "$ANISKIP_DIR/.git" ]; then
    info "ani-skip already cloned, pulling latest..."
    git -C "$ANISKIP_DIR" pull --ff-only 2>/dev/null && success "ani-skip updated" || warn "ani-skip update skipped (local changes?)"
else
    info "Cloning ani-skip into $ANISKIP_DIR ..."
    git clone --depth=1 "$ANISKIP_REPO" "$ANISKIP_DIR" || die "Failed to clone ani-skip"
    success "ani-skip cloned"
fi

# Install ani-skip binary
ANISKIP_BIN="$ANISKIP_DIR/ani-skip"
[ -f "$ANISKIP_BIN" ] || die "ani-skip binary not found at $ANISKIP_BIN — repo structure may have changed"
chmod +x "$ANISKIP_BIN"

# Symlink into BIN_DIR (overwrite if stale)
ln -sf "$ANISKIP_BIN" "$BIN_DIR/ani-skip"
success "ani-skip linked to $BIN_DIR/ani-skip"

# Verify it's callable
if command -v ani-skip >/dev/null 2>&1; then
    success "ani-skip is available in PATH"
else
    warn "ani-skip not found in PATH yet — you may need to restart your shell or source your rc file"
fi

# Write aniskip.lua
if [ -z "$MPV_CONF_DIR" ]; then
    warn "Platform '$PLATFORM' does not use mpv — skipping aniskip.lua install"
else
    SCRIPTS_DIR="$MPV_CONF_DIR/scripts"
    mkdir -p "$SCRIPTS_DIR"
    LUA_PATH="$SCRIPTS_DIR/aniskip.lua"

    info "Writing aniskip.lua to $LUA_PATH ..."
    cat > "$LUA_PATH" << 'EOF'
-- aniskip.lua — auto-skip OP/ED via ani-skip timestamps
-- Reads --script-opts=skip-op_start=N,skip-op_end=N,skip-ed_start=N,skip-ed_end=N
-- passed by ani-cli when launched with --skip flag
local mp  = require('mp')
local msg = require('mp.msg')

local options = { op_start=0, op_end=0, ed_start=0, ed_end=0 }
require('mp.options').read_options(options, 'skip')

msg.info(string.format(
    "aniskip loaded: op=[%s,%s] ed=[%s,%s]",
    options.op_start, options.op_end,
    options.ed_start, options.ed_end
))

local function skip_segment(_, time)
    if not time then return end
    if options.op_end > 0
        and time >= options.op_start
        and time <  options.op_end then
        msg.info("Skipping OP -> " .. options.op_end)
        mp.set_property_number("time-pos", options.op_end)
        return
    end
    if options.ed_end > 0
        and time >= options.ed_start
        and time <  options.ed_end then
        msg.info("Skipping ED -> " .. options.ed_end)
        mp.set_property_number("time-pos", options.ed_end)
    end
end

mp.observe_property("time-pos", "number", skip_segment)
EOF

    success "aniskip.lua written to $LUA_PATH"
fi

# Verify ani-skip works
info "Testing ani-skip with a known title (Cowboy Bebop ep 1)..."
TEST_OUT="$(ani-skip -q "Cowboy Bebop" -e 1 2>/dev/null || true)"
if printf "%s" "$TEST_OUT" | grep -q "skip-op_start"; then
    success "ani-skip test passed: $TEST_OUT"
else
    warn "ani-skip test returned no timestamps — network may be unavailable, or title not in DB"
    warn "This is non-fatal; skipping will work for titles that have skip data"
fi

# Done
printf "\n"
success "Setup complete!"
printf "  ${C_CYAN}ani-skip${C_RESET}    → $BIN_DIR/ani-skip\n"
[ -n "$MPV_CONF_DIR" ] && \
printf "  ${C_CYAN}aniskip.lua${C_RESET} → $MPV_CONF_DIR/scripts/aniskip.lua\n"
printf "\n"
info "You can now run: ${C_GREEN}./ani-cli --skip \"anime name\"${C_RESET}"
info "Or let ani-cli ask you interactively when you search."
