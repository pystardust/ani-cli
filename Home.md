Welcome to the ani-cli wiki!

ani-cli is a cli tool that scrapes Gogoanime and play's the anime locally on MPV (default) or VLC.

# Table of Contents
- [Flags](#Flags)
- [Examples](#Examples)
- [History](#History)
- [Tweaking](#Tweaking)
- [Issues](#Reporting-an-issue)
- [Contribute](#Developer-contribution)
- [Dependencies](#Dependencies)
- [Install](#Installation)
  - [Arch](#Arch)
  - [Linux](#Linux)
  - [Mac](#Mac)
  - [Windows](#Windows)
  - [Android](#Android)
- [Uninstall](#Uninstall)

## Flags
-c continue watching anime from history\
-a specify episode to watch\
-h show helptext\
-d download episode\
-p download episode to specified directory\
-q set video quality (best|worst|360|480|720|1080)\
-v use VLC as the media player\
-D delete history\
-U fetch update from github\
-V print version number and exit

## examples
`ani-cli` starts interactive mode.\
`ani-cli -v` starts interactive mode with vlc as media player.\
`ani-cli -a 300 bleach` starts episode 300 of bleach.\
`ani-cli -a 300 -v -q 720 bleach` starts episode 300 of bleach in vlc at 720p.

## History
The list of anime you have watched saves in `$XDG_CACHE_HOME/ani-hsts` or `$HOME/.cache/ani-hsts`\
The list is saved in the format "anime episode" for example bleach 200 ; each episode is on a new line.\
To add anime to the list either just watch or append to the list manually with a text editor. (IMPORTANT: if you'll be editing the list 
using a text editor, make sure to use proper tabs (2 spaces not 4), otherwise sed won't work)

## Tweaking
To set the defaults of the application there are some variables That can be changed under the `# default options` comment in `ani-cli` script.
1. player_fn this can be changed to your media player of choice (VLC, or MPV (default)) ; your media player of choice needs have a referrer title option, as the argument --referrer=https://gogoanime.film is necessary for functioning (else the video simply won't play)
2. quality this can be changed to any of these (best(default)|worst|360|480|720|1080)
3. download_dir can be changed to a custom directory of choice, by default it will be the working directory.


## Reporting an issue
1. Look at issues to see if the problem is not in discussion.
2. Follow the provided template.
3. Do not give bad one line information like "The program is broken".
4. Be respectful to the people trying to help you.
5. If you have a general question don't open an issue ask on the [Discord](https://discord.gg/aqu7GpqVmR)

## Developer contribution 
Follow the guidelines [here](https://github.com/pystardust/ani-cli/blob/master/CONTRIBUTING.md)

## Dependencies
- grep
- sed
- curl
- openssl
- mpv - Video Player
- aria2 - Download manager

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
*If you are upgrading from the old manual install process, you may have to remove the old ani-cli by running `sudo rm /usr/local/bin/ani-cli`*

### Windows

*Note that the installation instruction below must be done inside 
Powershell as **administrator**, not in Command Prompt*

Install scoop [(Guide)](https://scoop.sh/)
```
scoop bucket add extras
mkdir -p "$env:USERPROFILE/.cache"
scoop install ani-cli -g
```

*Make sure git bash is installed [(Install)](https://git-scm.com/download/win)*

*Run ani-cli in Git Bash (Running it in cmd or powershell may or may not work)*

### Android

Install termux [(Guide)](https://termux.com/)

```sh
pkg install git termux-tools ncurses-utils openssl-tool -y
git clone https://github.com/pystardust/ani-cli && cd ani-cli
cp ani-cli $PREFIX/bin/ani-cli
echo 'am start --user 0 -a android.intent.action.VIEW -d "$2" -n is.xyz.mpv/.MPVActivity' > $PREFIX/bin/mpv
chmod +x $PREFIX/bin/mpv
```

Install mpv-android [(Link)](https://play.google.com/store/apps/details?id=is.xyz.mpv)

*Add ```referrer="https://gogoanime.fi"``` to mpv.conf (Open mpv app, goto three dots top right->Settings->Advanced-->Edit mpv.conf)* 

*Note: VLC android doesn't support referrer option. So it will not work*

## Uninstall

* Arch Linux: ```yay -R ani-cli```
* Other Linux: Just remove the thing from path
* Mac: ```brew uninstall ani-cli```
* Windows: ```scoop uninstall ani-cli```
* Android: Just remove the thing from path


