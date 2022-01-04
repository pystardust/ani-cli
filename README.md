<p align="center"><img src="https://capsule-render.vercel.app/api?type=soft&fontColor=e5ab3e&text=pystardust/ani-cli&height=150&fontSize=60&desc=new and improved&descAlignY=75&descAlign=60&color=00000000&animation=twinkling"></p> 

[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](http://makeapullrequest.com)
[![Maintenance](https://img.shields.io/badge/Maintained%3F-yes-brightgreen.svg)](https://GitHub.com/pystardust/ani-cli/graphs/commit-activity)
[![Maintainer](https://img.shields.io/badge/maintainer-ura43-blue)](https://github.com/ura43)
[![Maintainer](https://img.shields.io/badge/maintainer-RayGL-blue)](https://github.com/RaynardGerraldo)
[![Maintainer](https://img.shields.io/badge/maintainer-Dink4n-blue)](https://github.com/Dink4n)
[![Maintainer](https://img.shields.io/badge/maintainer-CoolnsX-blue)](https://github.com/CoolnsX)
![Linux](https://img.shields.io/badge/os-linux-brightgreen)
![Mac](https://img.shields.io/badge/os-mac-yellow)
![Windows](https://img.shields.io/badge/os-windows-yellow)

A cli to browse and watch anime. This tool scrapes the site [gogoanime](https://gogoanime.pe).

> Please participate in the discussion on the scope of the project: [#273](https://github.com/pystardust/ani-cli/discussions/273)

## Usage
  ```
    ani-cli [-kv] [--dub] [-q <quality>] [-d | -p <download_dir>] [<query>]
    ani-cli [-kv] [--dub] [-q <quality>] -u | -n | -H
    ani-cli -h | -D
  Options:
    -u shows anime from history with unwatched episodes
    -n show recent anime
    -h show helptext
    -d download episode
    -H continue with next unwatched episode from history of watched series
    -D delete history
    -q set video quality (**best**|worst|360|480|720|1080)
    -k on keypress navigation (previous/next/replay/quit episode)
    --dub play the dub version if present
    -v use VLC as the media player

  Episode selection:
    Add 'h' on beginning for episodes like '6.5' -> 'h6'

    Multiple episodes can be chosen given a range
      Choose episode [1-13]: 1 6
      This would choose episodes 1 2 3 4 5 6
  ```

## Dependencies

### Essential
```
grep
curl
sed
aria2
git
```

### Optional
```
mpv - The default video player (recommended)
vlc - An alternative video player
```
  
## Install

### Linux / Mac
```sh
git clone https://github.com/pystardust/ani-cli.git
cd ani-cli
sudo make
```

### Windows
* Download and install [gitbash](https://git-scm.com/downloads)
* Download and install vlc (mpv needs further testing)
* Add vlc to Windows Env PATH like so: C:\Program Files\VideoLAN\VLC.
* Open git bash by right-clicking and choosing "Run as administrator"
* Run the following commands
```sh
git clone -b windows-vlc https://github.com/pystardust/ani-cli.git
cd ani-cli
chmod +x ani-cli-win
./install
```

## Disclaimer

The disclaimer of this project can be found [here.](./disclaimer.md)
