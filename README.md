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
<a href="https://github.com/justchokingaround"><img src="https://img.shields.io/badge/maintainer-justchokingaround-blue"></a>
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
   - [Debian](#Debian)
   - [Fedora](#Fedora)
   - [Arch](#Arch)
   - [OpenSuse Tumbleweed and Leap](#OpenSuse-Tumbleweed-and-Leap)
   - [From source](#Installing-from-source)
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

## Install

#### Users of V3.2 or the v3.2.x series should uninstall before upgrading
Otherwise you're likely to see an error like the following: ` "/usr/bin/ani-cli: line 470: (...)/player_mpv: No such file or directory"`

### Native packages

[![Packaging status](https://repology.org/badge/vertical-allrepos/ani-cli.svg?minversion=3.0)](https://repology.org/project/ani-cli/versions)

*Native packages have a more robust update cycle, but sometimes they are slow to upgrade. If the one for your platform is up-to-date we suggest going with it.*

### Linux

#### Debian

```
wget -qO- https://Wiener234.github.io/ani-cli-ppa/KEY.gpg | sudo tee /etc/apt/trusted.gpg.d/ani-cli.asc
wget -qO- https://Wiener234.github.io/ani-cli-ppa/ani-cli-debian.list | sudo tee /etc/apt/sources.list.d/ani-cli-debian.list
sudo apt update
sudo apt install ani-cli
```

#### Fedora

To install mpv (and vlc) you need _RPM Fusion free_ enabled. Simply follow the instructions here: https://rpmfusion.org/Configuration
To be able to install syncplay, you'll need to enable this copr repo (instructions included): https://copr.fedorainfracloud.org/coprs/batmanfeynman/syncplay/.

To install ani-cli:
```sh
sudo dnf copr enable derisis13/ani-cli
sudo dnf install ani-cli
```
*If for your distro uses rpm and you would like to see a native package, open an issue.*

#### Arch

Build and install from the AUR: 
```sh
yay -S ani-cli
```
Also consider ani-cli-git

#### OpenSuse Tumbleweed and Leap

On Suse the provided MPV and VLC packages are missing features that are used by ani-cli. The only required is the "Only Essentials" repository which has versions for each Suse release.
You can find instructions on this [here](https://en.opensuse.org/Additional_package_repositories#Packman).

To add the ani-cli copr repo, update then install ani-cli run (on both versions):
```sh
zypper addrepo https://download.copr.fedorainfracloud.org/results/derisis13/ani-cli/opensuse-tumbleweed-x86_64/ ani-cli
zypper dup
zypper install ani-cli
```
You'll get a warning about `Signature verification failed [4-Signatures public key is not available]` but this can be ignored from the prompt.

*Note: package is noarch, so any architecture should work, even though the repo is labled x86-64*

#### Installing from source

Install dependencies [(See below)](#Dependencies)

```sh
sudo rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" "/usr/local/bin/UI" /usr/local/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git" && cd ./ani-cli
sudo cp ./ani-cli /usr/local/bin
cd .. && rm -rf "./ani-cli"
```
*Also note that mpv installed through flatpak is not compatible*


### MacOS

Install dependencies [(See below)](#Dependencies)

Install [HomeBrew](https://docs.brew.sh/Installation) if not installed.

```sh
rm -rf "$(brew --prefix)/share/ani-cli" "$(brew --prefix)/bin/ani-cli" "$(brew --prefix)/bin/UI" "$(brew --prefix)"/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git" && cd ./ani-cli
cp ./ani-cli "$(brew --prefix)"/bin 
cd .. && rm -rf ./ani-cli
```

*To install (with Homebrew) the dependencies required on Mac OS, you can run:* 

```sh
brew install curl grep axel openssl@1.1 ffmpeg git && \
brew install --cask iina
``` 
*Why iina and not mpv? Drop-in replacement for mpv for MacOS. Integrates well with OSX UI. Excellent support for M1. Open Source.*  

### Windows

*Make sure git bash is installed [(Install)](https://git-scm.com/download/win)*

*Note that the installation instruction below must be done inside **Git Bash**, not in Command Prompt or Powershell*

mpv is not added to $PATH automatically when installed and thus the script is unable to use it. You either have to do this manually, or install it via scoop (recommended):
```sh
scoop install mpv
```

#### Scoop bucket

```sh
scoop bucket add extras
scoop install ani-cli
```

#### From source
```sh
rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" "/usr/local/bin/UI" /usr/local/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git" && cd ./ani-cli
cp ./ani-cli /usr/bin
cd .. && rm -rf ./ani-cli
```

*Run ani-cli in Git Bash (Running it in cmd or powershell may or may not work)*

### Android

Install termux [(Guide)](https://termux.com/)

#### Termux package

```sh
pkg up -y
pkg install ani-cli
```

#### From source

```sh
pkg up -y
rm -rf "$PREFIX/share/ani-cli" "$PREFIX/bin/ani-cli" "$PREFIX/bin/UI" "$PREFIX"/local/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git" && cd ./ani-cli
cp ./ani-cli "$PREFIX"/bin
cd .. && rm -rf ./ani-cli
```

Note : Vlc Android now works too ;)

You need to add any referrer in mpv by opening mpv [(playstore version)](https://play.google.com/store/apps/details?id=is.xyz.mpv), going into Settings -> Advanced -> Edit mpv.conf and adding (for example):

```
referrer="https://animixplay.to/"
```

### Steam Deck (draft) (not tested yet)

#### Copypaste script:

1. switch to Desktop mode:
2. open `Konsole`
3. Copy the script and paste it in the CLI

```
git clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
~/.fzf/install

mkdir ~/.aria2
wget -O ~/.aria2/aria2-1.36.0.tar.gz https://github.com/aria2/aria2/releases/download/release-1.36.0/aria2-1.36.0.tar.gz
tar xzf ~/.aria2/aria2-1.36.0.tar.gz -C ~/.local/bin/

git clone https://github.com/pystardust/ani-cli.git ~/.ani-cli
cp ~/.ani-cli/ani-cli ~/.local/bin/

flatpak install io.mpv.Mpv

```
#### Installation in steps:

##### Install mpv (Flatpak version)

```
flatpak install io.mpv.Mpv
```

##### Install [fzf](https://github.com/junegunn/fzf): 

```
git clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
~/.fzf/install
```

##### Install [aria2](https://github.com/aria2/aria2) (for --download):

```
mkdir ~/.aria2
wget -O ~/.aria2/aria2-1.36.0.tar.gz https://github.com/aria2/aria2/releases/download/release-1.36.0/aria2-1.36.0.tar.gz
tar xzf ~/.aria2/aria2-1.36.0.tar.gz -C ~/.local/bin/

```

##### Install ani-cli:

```
git clone https://github.com/pystardust/ani-cli.git ~/.ani-cli
cp ~/.ani-cli/ani-cli ~/.local/bin/

```

Note: packman/AUR installation does work, but it would be wiped with the next SteamOS update.

## Uninstall

* apt:
```
sudo apt remove ani-cli
# to remove the repository from apt:
sudo rm -f /etc/apt/trusted.gpg.d/ani-cli.asc /etc/apt/sources.list.d/ani-cli-debian.list
```
* dnf:
```
sudo dnf remove ani-cli      # for ani-cli
# disable the repo in dnf
dnf copr disable derisis13/ani-cli
```
You might want to uninstall RPM fusion if you don't use it otherwise
* zypper:
```sh
zypper remove ani-cli
zypper removerepo ani-cli
```
You might want to remove `packman-essentials` if you don't need it otherwise
* AUR:
```
yay -R ani-cli
```
* Scoop:
```sh
scoop uninstall ani-cli
```
* Linux:  
```sh
sudo rm "/usr/local/bin/ani-cli"
```
* Mac:  
```sh
rm "$(brew --prefix)/bin/ani-cli"
```
* Windows:
In **Git Bash** run (as administrator):
```sh
rm "/usr/bin/ani-cli"
```
* Termux package
```
pkg remove ani-cli
```
* Android:
```sh
rm "$PREFIX/bin/ani-cli"
```

## Dependencies

- grep
- sed
- awk
- curl
- wget
- openssl
- mpv - Video Player
- iina - mpv replacement for MacOS
- aria2c - Download manager
- ffmpeg - m3u8 Downloader
- fzf - User interface

## Homies 

* [animdl](https://github.com/justfoolingaround/animdl): Ridiculously efficient, fast and light-weight (supports most sources: animixplay, 9anime...) (Python)
* [anime-helper-shell](https://github.com/Atreyagaurav/anime-helper-shell): A python shell for searching, watching, and downloading anime (Python)
* [anipy-cli](https://github.com/sdaqo/anipy-cli): ani-cli rewritten in python (Python)
* [dra-cla](https://github.com/CoolnsX/dra-cla): ani-cli equivalent for korean dramas (Shell)
* [kaa.si-cli](https://github.com/Soviena/kaa.si-cli): Stream anime from kaa.si and sync with anilist (Python)
* [lobster](https://github.com/justchokingaround/lobster): Life action movies and series fom the terminal (Shell)
* [manga-cli](https://github.com/7USTIN/manga-cli): Read manga in the cli (Shell)
* [mangal](https://github.com/metafates/mangal): Download & read manga from any source with anilist sync (Go)
* [mov-cli](https://github.com/mov-cli/mov-cli): Watch movies/tv shows in the cli (work in progress) (Python/Shell)
* [saikou](https://github.com/saikou-app/saikou): Best android app for anime/manga with anilist integration (Kotlin)
* [tv-cli](https://github.com/Spaxly/tv-cli): Watch live TV in the cli (Shell)
