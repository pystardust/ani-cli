<p align="center"><img src="https://capsule-render.vercel.app/api?type=soft&fontColor=e5ab3e&text=pystardust/ani-cli&height=150&fontSize=60&desc= beautiful, documented and portable.&descAlignY=75&descAlign=60&color=00000000&animation=twinkling"></p> 

A cli to browse and watch anime. This tool scrapes the site [gogoanime](https://gogoanime.pe).

## Status

> Script currently broken on mac, working on linux
> 
> Thank you @RaynardGerraldo for fixing the crippling issues 221 and 202

## Usage
  ```
  ani-cli (OPTION) (query)

  Options
    -h show helptext
    -d download episode
    -H continue where you left off
    -D delete history
    -q set video quality (**best**/worst/360/480/720/1080)
    --dub play the dub version if present
    -v use VLC as the media player
  
  Multiple episodes can be chosen given a range
    Choose episode [1-13]: 1 6
    This would choose episodes 1 2 3 4 5 6
  ```
  
## Install

### Dependencies

* grep
* curl
* sed
* mpv
* ffmpeg

### Linux

```sh
git clone https://github.com/pystardust/ani-cli.git
cd ani-cli
sudo make
```
### Mac
*tba*

### Windows
*tba*

### Other
*tba*



## Misc

- Windows instructions can be found in this branch https://github.com/pystardust/ani-cli/tree/windows-vlc
