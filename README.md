<p align=center>
<br>
<a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
<img src="https://img.shields.io/badge/os-linux-brightgreen">
<img src="https://img.shields.io/badge/os-mac-brightgreen">
<img src="https://img.shields.io/badge/os-windows-brightgreen">
<img src="https://img.shields.io/badge/os-android-brightgreen">
<br>
<h1 align="center">
<a href="https://matrix.to/#/#ani-cli:matrix.org"><img src="https://element.io/blog/content/images/2020/07/Logomark---white-on-green.png" width="80"></a>
<a href="https://discord.gg/aqu7GpqVmR"><img src="https://pnggrid.com/wp-content/uploads/2021/05/Discord-Logo-Square-1024x1024.png" width="80"></a>
<a href="https://nightly.revolt.chat/invite/4FKHbs78"><img src="https://developers.revolt.chat/img/logo.png" width="80"></a>
<br>
<a href="https://github.com/port19x"><img src="https://img.shields.io/badge/lead-port19x-lightblue"></a>
<a href="https://github.com/CoolnsX"><img src="https://img.shields.io/badge/maintainer-CoolnsX-blue"></a>
<a href="https://github.com/RaynardGerraldo"><img src="https://img.shields.io/badge/maintainer-RayGL-blue"></a>
<br>
<a href="https://github.com/71zenith"><img src="https://img.shields.io/badge/maintainer-71zenith-blue"></a>
<a href="https://github.com/iamchokerman"><img src="https://img.shields.io/badge/maintainer-iamchokerman-blue"></a>
<a href="https://github.com/Derisis13"><img src="https://img.shields.io/badge/maintainer-Derisis13-blue"></a>

</p>

<h3 align="center">
A cli to browse and watch anime (alone AND with friends). This tool scrapes the site <a href="https://animixplay.to/">animixplay.</a>
</h3>
	
<h1 align="center">
	Showcase
</h1>

https://user-images.githubusercontent.com/44473782/160729779-41fe207c-b5aa-4fed-87db-313c83caf6bb.mp4

## Table of Contents

- [Fixing errors](#Fixing-errors)
- [New in v3](#New-in-v3)
- [Install](#Install)
  - [Linux](#Linux)
  - [MacOS](#MacOS)
  - [Windows](#Windows)
  - [Android](#Android)
- [Uninstall](#Uninstall)
- [Dependencies](#Dependencies)
- [Homies](#Homies)
- [Contribution Guidelines](./CONTRIBUTING.md)
- [Disclaimer](./disclaimer.md)

## Fixing errors

If you encounter "Video url not found" or any breaking issue, then make sure you are on latest version by typing
`sudo ani-cli -U` to update on Linux, Mac and Android. On Windows, run gitbash as administrator then there type `ani-cli -U`.
If after this the issue persists then open an issue.
<br>  
If after updating you get the following error: ` "/usr/bin/ani-cli: line 470: /usr/bin/players/player_mpv: No such file or directory"` then uninstall and reinstall ani-cli with the installation instructions provided below.

## New in v3
```txt
We now scrape animixplay instead of gogoanime, which allows for faster link fetching as well as getting new 
releases sooner.

New arguments:
-f select provider to scrape first
-x print all video links from all providers to stdout (for debugging purpose)

To see a list with all the arguments, use the -h or --help argument
```

## Install
# IMPORTANT: Please uninstall ani-cli before proceeding.
#### ani-cli V3 has breaking changes and is incompatible with V2's install location. Plasase uninstall before proceeding.

### Native packages

[![Packaging status](https://repology.org/badge/vertical-allrepos/ani-cli.svg)](https://repology.org/project/ani-cli/versions)

### Linux

Install dependencies [(See below)](#Dependencies)

```sh
sudo rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" && \
sudo mkdir -p "/usr/local/share/" "/usr/local/bin/" && \
sudo git clone -b "master" "https://github.com/pystardust/ani-cli.git" "/usr/local/share/ani-cli" && \
sudo ln -sf "/usr/local/share/ani-cli/ani-cli" "/usr/local/bin/ani-cli"
```
*Also note that mpv installed through flatpak is not compatible*

### MacOS

Install dependencies [(See below)](#Dependencies)

Install [HomeBrew](https://docs.brew.sh/Installation) if not installed.

```sh
rm -rf "$(brew --prefix)/share/ani-cli" "$(brew --prefix)/bin/ani-cli" && \
git clone -b "master" "https://github.com/pystardust/ani-cli.git" "$(brew --prefix)/share/ani-cli" && \
ln -sf "$(brew --prefix)/share/ani-cli/ani-cli" "$(brew --prefix)/bin/ani-cli"
```

*To install (with Homebrew) the dependencies required on Mac OS, you can run:* 

```sh
brew install curl grep aria2 openssl@1.1 ffmpeg git && \
brew install --cask iina
``` 
*Why iina and not mpv? Drop-in replacement for mpv for MacOS. Integrates well with OSX UI. Excellent support for M1. Open Source.*  

### Windows

*Make sure git bash is installed [(Install)](https://git-scm.com/download/win)*

*Note that the installation instruction below must be done inside **Git Bash**, not in Command Prompt or Powershell*

```sh
rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" && \
mkdir -p "/usr/local/share/" "/usr/local/bin/" && \
git clone -b "master" "https://github.com/pystardust/ani-cli.git" "/usr/local/share/ani-cli" && \
ln -sf "/usr/local/share/ani-cli/ani-cli" "/usr/local/bin/ani-cli"
```

*Run ani-cli in Git Bash (Running it in cmd or powershell may or may not work)*

### Android

Install termux [(Guide)](https://termux.com/)

```sh
rm -rf "$PREFIX/share/ani-cli" "$PREFIX/bin/ani-cli" && \
mkdir -p "$PREFIX/share/ani-cli" "$PREFIX/bin/ani-cli" && \
git clone -b "master" "https://github.com/pystardust/ani-cli.git" "$PREFIX/share/ani-cli" && \
ln -sf "$PREFIX/share/ani-cli/ani-cli" "$PREFIX/bin/ani-cli"
```
For doodstream to work you need to add any referrer in mpv by opening mpv [(playstore version)](https://play.google.com/store/apps/details?id=is.xyz.mpv), going into Settings -> Advanced -> Edit mpv.conf and adding (for example):

```
referrer="https://gogoanime.fi/"
```
Make sure to update your packages:

```sh
pkg up
```

In the case mpv only plays audio, you can try running this command:
```sh
echo 'am start --user 0 -a android.intent.action.VIEW -d "$1" -n is.xyz.mpv/.MPVActivity' > $PREFIX/bin/mpv
```


## Uninstall

* Linux:  
```sh
sudo rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli"
```
* Mac:  
```sh
rm -rf "$(brew --prefix)/share/ani-cli" "$(brew --prefix)/bin/ani-cli"
```
* Windows:
In **Git Bash** run:
```sh
rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli"
```
* Android:  
```sh
rm -rf "/data/data/com.termux/files/usr/share/ani-cli" "/data/data/com.termux/files/usr/bin/ani-cli"
```

## Dependencies

- grep
- sed
- awk
- curl
- openssl
- mpv - Video Player
- iina - mpv replacement for MacOS
- aria2 - Download manager
- ffmpeg - m3u8 Downloader

## Homies 

* [animdl](https://github.com/justfoolingaround/animdl): Ridiculously efficient, fast and light-weight (supports most sources: animixplay, 9anime...) (Python)
* [anime-helper-shell](https://github.com/Atreyagaurav/anime-helper-shell): A python shell for searching, watching, and downloading anime (Python)
* [anipy-cli](https://github.com/sdaqo/anipy-cli): ani-cli rewritten in python (Python)
* [dra-cla](https://github.com/CoolnsX/dra-cla): ani-cli equivalent for korean dramas (Shell)
* [kaa.si-cli](https://github.com/Soviena/kaa.si-cli): Stream anime from kaa.si and sync with anilist (Python)
* [manga-cli](https://github.com/7USTIN/manga-cli): Read manga in the cli (Shell)
* [mov-cli](https://github.com/mov-cli/mov-cli): Watch movies/tv shows in the cli (work in progress) (Python/Shell)
* [saikou](https://github.com/saikou-app/saikou): Best android app for anime/manga with anilist integration (Kotlin)
