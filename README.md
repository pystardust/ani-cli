<p align=center>
<br>
<a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
<img src="https://img.shields.io/badge/os-linux-brightgreen">
<img src="https://img.shields.io/badge/os-mac-brightgreen"></a>
<img src="https://img.shields.io/badge/os-windows-brightgreen"></a>
<img src="https://img.shields.io/badge/os-android-brightgreen"></a>
<br>
<a href="https://discord.gg/aqu7GpqVmR"><img src="https://invidget.switchblade.xyz/aqu7GpqVmR"></a>
<br>
<a href="https://github.com/CoolnsX"><img src="https://img.shields.io/badge/maintainer-CoolnsX-blue"></a>
<a href="https://github.com/RaynardGerraldo"><img src="https://img.shields.io/badge/maintainer-RayGL-blue"></a>
<br>
<a href="https://github.com/71zenith"><img src="https://img.shields.io/badge/maintainer-71zenith-blue"></a>
<a href="https://github.com/iamchokerman"><img src="https://img.shields.io/badge/maintainer-iamchokerman-blue"></a>
<a href="https://github.com/Derisis13"><img src="https://img.shields.io/badge/maintainer-Derisis13-blue"></a>

</p>

A cli to browse and watch anime. This tool scrapes the site [gogoanime](https://gogoanime.pe).

<h1 align="center">
	Showcase
</h1>
<p align="center">
<img src=.assets/ani-cli.gif width="100%">
</p>

## Table of Contents
- [Install](#Installation)
  - [Arch](#Arch)
  - [Linux](#Linux)
  - [Mac](#Mac)
  - [Windows](#Windows)
  - [Android](#Android)
- [Uninstall](#Uninstall)
- [Dependencies](#Dependencies)
- [Contribution Guidelines](./CONTRIBUTING.md)
- [Disclaimer](./disclaimer.md)

## Install

### Arch

Also consider ani-cli-git

```sh
yay -S ani-cli
```

### Linux

Install dependencies [(See below)](#Dependencies)

```sh
git clone https://github.com/pystardust/ani-cli && cd ani-cli
sudo cp ani-cli /usr/local/bin/ani-cli
```

*Note that mpv installed through flatpak is not compatible*

### Mac

Install homebrew [(Guide)](https://brew.sh/)

```sh
brew tap iamchokerman/ani-cli
brew install ani-cli
```

### Windows
*Note that the installation must be done inside powershell, not in cmd*

Install scoop [(Guide)](https://scoop.sh/)

```
scoop bucket add extras
mkdir -p "$env:USERPROFILE/.cache"
scoop install ani-cli
```

*Make sure git bash is installed [(Install)](https://git-scm.com/download/win)*

*You can run ani-cli from any Windows console, e.g Windows Terminal,Command Prompt,Powershell*

### Android

Install termux [(Guide)](https://termux.com/)

```sh
pkg install git make termux-tools ncurses-utils openssl-tool -y
git clone https://github.com/pystardust/ani-cli && cd ani-cli
cp ani-cli $PREFIX/bin/ani-cli
echo 'termux-open "$2"' > $PREFIX/bin/mpv
```

## Uninstall

* Arch Linux: ```yay -R ani-cli```
* Other Linux: Just remove the thing from path
* Mac: ```brew uninstall ani-cli```
* Windows: ```scoop uninstall ani-cli```
* Android: Just remove the thing from path

## Dependencies

- grep
- sed
- curl
- openssl
- mpv - Video Player
- aria2 - Download manager
