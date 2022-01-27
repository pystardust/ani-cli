<p align=center>
<img src="https://capsule-render.vercel.app/api?type=soft&fontColor=e5ab3e&text=pystardust/ani-cli&height=150&fontSize=60&desc=now%20sponsoring%20shellcheck&descAlignY=75&descAlign=60&color=00000000&animation=twinkling">
<br>
<a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
<img src="https://img.shields.io/badge/os-linux-brightgreen">
<img src="https://img.shields.io/badge/os-mac-brightgreen"></a>
<img src="https://img.shields.io/badge/os-windows-brightgreen"></a>
<img src="https://img.shields.io/badge/os-android-brightgreen"></a>
<br>
<a href="https://discord.gg/aqu7GpqVmR"><img src="https://invidget.switchblade.xyz/aqu7GpqVmR"></a>
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
    ani-cli [-v | -i] [-q <quality>] [-e <arguments>] [-a <episode>] [-d | -p <download_dir>] [<query>]
    ani-cli [-v | -i] [-q <quality>] [-e <arguments>] -c
    ani-cli -h | -D | -U | -V

  Options:
    -c continue watching anime from history
    -e pass arguments to player/downloader
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
  ```

## Dependencies

### Essential

```text
grep
sed
curl
openssl
```

### Optional

```text
mpv - The default video player (recommended)
aria2 - For downloading (recommended)
iina - The recommended video player for Mac
vlc - An alternative video player
diff - Update checking
patch - Update checking
```

## Install

### Linux

```sh
git clone https://github.com/pystardust/ani-cli
cd ani-cli
sudo make
```

### Mac

```sh
git clone https://github.com/pystardust/ani-cli
cd ani-cli
sudo make
```

### Android/Termux

```sh
apt update
apt install git make ncurses-utils openssl-tool -y
git clone https://github.com/pystardust/ani-cli
cd ani-cli
make
```

### Windows

* Download and install dependencies as mentioned above
* If you choose vlc, add it to path [Guide](https://www.vlchelp.com/add-vlc-command-prompt-windows)
* Download and install [gitbash](https://git-scm.com/downloads)
* Open git bash by right-clicking and choosing "Run as administrator"
* Run the following commands

```sh
git clone https://github.com/pystardust/ani-cli.git
cd ani-cli
make
```

## Disclaimer

The disclaimer of this project can be found [here](./disclaimer.md)

