<p align="center"><img src="https://capsule-render.vercel.app/api?type=soft&fontColor=e5ab3e&text=pystardust/ani-cli&height=150&fontSize=60&desc=supersonic%20development%20speed&descAlignY=75&descAlign=60&color=00000000&animation=twinkling"></p>

[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](http://makeapullrequest.com)
[![Maintenance](https://img.shields.io/badge/Maintained%3F-yes-brightgreen.svg)](https://GitHub.com/pystardust/ani-cli/graphs/commit-activity)
[![Maintainer](https://img.shields.io/badge/maintainer-pystardust-blue)](https://github.com/pystardust)
[![Maintainer](https://img.shields.io/badge/maintainer-ura43-blue)](https://github.com/ura43)
[![Maintainer](https://img.shields.io/badge/maintainer-RayGL-blue)](https://github.com/RaynardGerraldo)
[![Maintainer](https://img.shields.io/badge/maintainer-Dink4n-blue)](https://github.com/Dink4n)
[![Maintainer](https://img.shields.io/badge/maintainer-CoolnsX-blue)](https://github.com/CoolnsX)
![Linux](https://img.shields.io/badge/os-linux-brightgreen)
![Mac](https://img.shields.io/badge/os-mac-brightgreen)
![Windows](https://img.shields.io/badge/os-windows-yellow)
![Android](https://img.shields.io/badge/os-android-yellow)


<p align="center">
<a href="https://discord.gg/aqu7GpqVmR">
<img src="https://invidget.switchblade.xyz/aqu7GpqVmR">
</a></p>

A cli to browse and watch anime. This tool scrapes the site [gogoanime](https://gogoanime.pe).

## Usage

  ```text
    ani-cli [-v | -i] [-q <quality>] [-s] [-d | -p <download_dir>] [<query>]
    ani-cli [-v | -i] [-q <quality>] -c
    ani-cli -h | -D | -U | -V
    
  Options:
    -c continue watching anime from history
    -h show helptext
    -s speedup download on low bandwidth and low cpu power
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
jq
```

### Optional

```text
mpv - The default video player (recommended)
aria2 - For downloading (recommended)
iina - An alternative video player for the Mac
vlc - An alternative video player
diff - Update checking
patch - Update checking
```

## Install

### Linux / Mac

```sh
git clone https://github.com/pystardust/ani-cli
cd ani-cli
sudo make
```

### Android (Termux)

```sh
apt update
apt install git make ncurses-utils openssl-tool jq -y
git clone https://github.com/pystardust/ani-cli
cd ani-cli
make
```

### Windows

* Download and install dependencies as mentioned above
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
