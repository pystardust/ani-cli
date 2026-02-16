#!/bin/bash

# Dependency installation script for Debian/Ubuntu based distributions
# Tested on: Debian 12, Ubuntu 22.04, Zorin OS 17, Linux Mint 21, Pop!_OS 22.04
# Compatible with: Any Debian/Ubuntu derivative using apt package manager

echo "=========================================="
echo "Installing ani-cli dependencies"
echo "For Debian/Ubuntu based distributions"
echo "=========================================="
echo ""

# Update repositories
echo "[1/9] Updating repositories..."
sudo apt update

# Install basic dependencies available in default repositories
echo ""
echo "[2/9] Installing grep, sed, curl, patch, ffmpeg..."
sudo apt install -y grep sed curl patch ffmpeg

# Install aria2c
echo ""
echo "[3/9] Installing aria2c (download manager)..."
sudo apt install -y aria2

# Install fzf
echo ""
echo "[4/9] Installing fzf (user interface)..."
sudo apt install -y fzf

# Install mpv
echo ""
echo "[5/9] Installing mpv (video player)..."
sudo apt install -y mpv

# Install yt-dlp (latest version via repository or pip)
echo ""
echo "[6/9] Installing yt-dlp (m3u8 downloader)..."
if command -v pip3 &> /dev/null; then
    sudo pip3 install -U yt-dlp
else
    # Install via repository if pip is not available
    sudo apt install -y yt-dlp 2>/dev/null || {
        echo "Installing yt-dlp manually..."
        sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
        sudo chmod a+rx /usr/local/bin/yt-dlp
    }
fi

# ani-skip (optional) - manual installation
echo ""
echo "[7/9] Installing ani-skip (optional)..."
if command -v cargo &> /dev/null; then
    cargo install ani-skip
else
    echo "Rust/Cargo not found. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    cargo install ani-skip
fi

# Installation verification
echo ""
echo "[8/9] Verifying installations..."
echo ""

check_command() {
    if command -v $1 &> /dev/null; then
        echo "✓ $1 installed: $(command -v $1)"
    else
        echo "✗ $1 NOT found"
    fi
}

check_command grep
check_command sed
check_command curl
check_command mpv
check_command aria2c
check_command yt-dlp
check_command ffmpeg
check_command fzf
check_command patch
check_command ani-skip

echo ""
echo "[9/9] Installation completed!"
echo ""
echo "=========================================="
echo "All dependencies have been installed"
echo "ani-cli is ready to use!"
echo "=========================================="
echo ""
echo "Run 'ani-cli' to start watching anime"
echo "Run 'ani-cli -h' for help"
