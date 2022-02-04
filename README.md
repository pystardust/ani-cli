<p align=center>
<img src="https://user-images.githubusercontent.com/82055622/152351606-308b770d-a47e-4b92-b161-e21b4eff49e6.png">
<br>
<a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
<img src="https://img.shields.io/badge/os-linux-brightgreen">
<img src="https://img.shields.io/badge/os-mac-brightgreen"></a>
<img src="https://img.shields.io/badge/os-windows-brightgreen"></a>
<img src="https://img.shields.io/badge/os-android-brightgreen"></a>
<br>
<a href="https://discord.gg/aqu7GpqVmR"><img src="https://invidget.switchblade.xyz/aqu7GpqVmR"></a>
<br>
<a href="https://github.com/ura43"><img src="https://img.shields.io/badge/lead-ura43-lightblue"></a>
<a href="https://github.com/CoolnsX"><img src="https://img.shields.io/badge/maintainer-CoolnsX-blue"></a>
<a href="https://github.com/RaynardGerraldo"><img src="https://img.shields.io/badge/maintainer-RayGL-blue"></a>
<br>
<a href="https://github.com/71zenith"><img src="https://img.shields.io/badge/maintainer-71zenith-blue"></a>
<a href="https://github.com/iamchokerman"><img src="https://img.shields.io/badge/maintainer-iamchokerman-blue"></a>
<a href="https://github.com/Derisis13"><img src="https://img.shields.io/badge/maintainer-Derisis13-blue"></a>

</p>

A cli to browse and watch anime. This tool scrapes the site [gogoanime](https://gogoanime.pe).

## Table of Contents
- [Usage](#Usage)
- [Dependencies](#Dependencies)
- [Installation](#Installation)
  - [Linux](#Linux)
  - [Mac](#Mac)
  - [Android/Termux](#Android/Termux)
  - [Windows](#Windows)
- [Contribution Guidelines](./CONTRIBUTING.md)
- [Disclaimer](./disclaimer.md)

## Usage

  ```text
    ani-cli [-v | -i] [-q <quality>] [-a <episode>] [-d | -p <download_dir>] [<query>]
    ani-cli [-v | -i] [-q <quality>] -c
    ani-cli -h | -D | -U | -V

  Options:
    -c continue watching anime from history
    -a specify episode to watch
    -h show helptext
    -d download episode
    -q set video quality (best|worst|360|480|720|1080)
    -i use iina as the media player
    -v use VLC as the media player
    -D delete history
    -U fetch update from github
    -V print version number and exit

  Episode selection:
    Add 'h' on beginning for episodes like '6.5' -> 'h6'

    Multiple episodes can be chosen given a range
      Choose episode [1-13]: 1 6
      This would choose episodes 1 2 3 4 5 6
      To select the last episode use "-1"
  ```

## Dependencies

- grep
- sed
- curl
- openssl
- mpv - Video Player
- aria2 - Download manager
- diff - Update checking
- patch - Update checking

## Install

### Linux

#### Arch Linux

```sh
yay -S ani-cli
```

#### Other

```sh
git clone https://github.com/pystardust/ani-cli && cd ani-cli
sudo cp ani-cli /usr/local/bin/ani-cli
```

### Mac (with Homebrew)

```sh
brew tap iamchokerman/ani-cli
brew install ani-cli
```

### Android/Termux

```sh
pkg install git make termux-tools ncurses-utils openssl-tool -y
git clone https://github.com/pystardust/ani-cli && cd ani-cli
cp ani-cli $PREFIX/bin/ani-cli
echo 'termux-open "$2"' > $PREFIX/bin/mpv
```

### Windows

Install scoop [(Guide)](https://scoop.sh/)
```
scoop bucket add extras
mkdir -p "$env:USERPROFILE/.cache"
scoop install ani-cli -g
```
*Ani-cli only runs in git bash, not powershell*

## Uninstall
* Just remove the thing from path lul
* Mac: ```brew uninstall ani-cli```
* Windows: ```scoop uninstall ani-cli```
