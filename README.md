<p align=center>
<br>
<a href="http://makeapullrequest.com"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
<a href="#Linux"><img src="https://img.shields.io/badge/os-linux-brightgreen">
<a href="#MacOS"><img src="https://img.shields.io/badge/os-mac-brightgreen">
<a href="#Windows"><img src="https://img.shields.io/badge/os-windows-brightgreen">
<a href="#Android"><img src="https://img.shields.io/badge/os-android-brightgreen">
<a href="#iOS"><img src="https://img.shields.io/badge/os-ios-brightgreen">
<a href="#Steam-deck"><img src="https://img.shields.io/badge/os-steamdeck-brightgreen">
<br>
<h1 align="center">
<a href="https://discord.gg/aqu7GpqVmR">
<img src="https://invidget.switchblade.xyz/aqu7GpqVmR">
</a>
<br>
<a href="https://github.com/port19x"><img src="https://img.shields.io/badge/lead-port19x-lightblue"></a>
<a href="https://github.com/CoolnsX"><img src="https://img.shields.io/badge/maintainer-CoolnsX-blue"></a>
<a href="https://github.com/justchokingaround"><img src="https://img.shields.io/badge/maintainer-justchokingaround-blue"></a>
<a href="https://github.com/Derisis13"><img src="https://img.shields.io/badge/maintainer-Derisis13-blue"></a>
<a href="https://github.com/71zenith"><img src="https://img.shields.io/badge/maintainer-71zenith-blue"></a>

</p>

<h3 align="center">
A cli to browse and watch anime (alone AND with friends). This tool scrapes the site <a href="https://allanime.to/">allanime.</a>
</h3>
	
<h1 align="center">
	Showcase
</h1>

[ani-cli-demo.webm](https://user-images.githubusercontent.com/44473782/224679247-0856e652-f187-4865-bbcf-5a8e5cf830da.webm)

## Table of Contents

- [Fixing errors](#Fixing-errors)
- [Install](#Install)
  - [Linux](#Linux)
   - [Debian](#Debian-unstable)
   - [Fedora](#Fedora)
   - [Arch](#Arch)
   - [OpenSuse Tumbleweed and Leap](#OpenSuse-Tumbleweed-and-Leap)
   - [From source](#Installing-from-source)
  - [MacOS](#MacOS)
  - [Windows](#Windows)
  - [Android](#Android)
  - [Steam Deck](#Steam-deck)
  - [iOS](#iOS)
- [Uninstall](#Uninstall)
- [Dependencies](#Dependencies)
- [Homies](#Homies)
- [Contribution Guidelines](./CONTRIBUTING.md)
- [Disclaimer](./disclaimer.md)

## Fixing errors

If you encounter "Video url not found" or any breaking issue, then make sure you are on latest version by typing
`sudo ani-cli -U` to update on Linux, Mac and Android. On Windows, run gitbash as administrator then there type `ani-cli -U`.
If after this the issue persists then open an issue.

History has been reworked and relocated. We're working on a transition script, please be patient. Old history can be viewed with `less ${XDG_CACHE_HOME:-$HOME/.cache}/ani-hsts`

## Install

#### Users of V3.2 or the v3.2.x series should uninstall before upgrading
Otherwise you're likely to see an error like the following: ` "/usr/bin/ani-cli: line 470: (...)/player_mpv: No such file or directory"`

### Native packages

[![Packaging status](https://repology.org/badge/vertical-allrepos/ani-cli.svg?minversion=4.0)](https://repology.org/project/ani-cli/versions)

*Native packages have a more robust update cycle, but sometimes they are slow to upgrade. If the one for your platform is up-to-date we suggest going with it.*

### Linux

#### Debian unstable

```sh
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
Also consider `ani-cli-git`

#### Gentoo

Build and install from the GURU:
```sh
sudo eselect repository enable guru
sudo emaint sync -r guru
sudo emerge -a ani-cli
```
Consider using the 9999 ebuild.
```sh
sudo emerge -a =app-misc/ani-cli-9999
```

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

*Note: package is noarch, so any architecture should work, even though the repo is labelled x86-64*

#### Installing from source

Install dependencies [(See below)](#Dependencies)

```sh
sudo rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" "/usr/local/bin/UI" /usr/local/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git"
sudo cp ani-cli/ani-cli /usr/local/bin
rm -rf ani-cli
```

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
brew install curl grep aria2 ffmpeg git fzf yt-dlp && \
brew install --cask iina
```
*Why iina and not mpv? Drop-in replacement for mpv for MacOS. Integrates well with OSX UI. Excellent support for M1. Open Source.*

### Windows

*ani-cli needs a posix shell and the current way is git bash. Unfortunately fzf can't run in git bash's default terminal. The solution is to use git bash in windows terminal*

First, you'll need windows terminal preview. [(Install)](https://apps.microsoft.com/store/detail/windows-terminal-preview/9N8G5RFZ9XK3?hl=de-at&gl=at&rtc=1)

Then make sure git bash is installed. [(Install)](https://git-scm.com/download/win) It needs to be added to windows terminal [(Instructions)](https://stackoverflow.com/questions/56839307/adding-git-bash-to-the-new-windows-terminal)

The following steps and ani-cli need to be run from git bash in windows terminal.

#### Scoop bucket

```sh
scoop bucket add extras
scoop install ani-cli
```

#### From source
```sh
rm -rf "/usr/local/share/ani-cli" "/usr/local/bin/ani-cli" "/usr/local/bin/UI" /usr/local/bin/player_* #If some of these aren't found, it's not a problem
git clone "https://github.com/pystardust/ani-cli.git"
cp ani-cli/ani-cli /usr/bin
rm -rf ani-cli
```

#### Dependencies

All dependencies can be installed with scoop (from the extras bucket), however some users experienced that installed programs aren't always added to the path. If this happens installing from winget instead usually works.

Note that curl can cause issues.
ani-cli has been tested unsuccessfully with curl `7.83.1` and successfully with `7.86.0`.
If you run into issues, try the scoop install or grab the newest curl you can find.

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
git clone "https://github.com/pystardust/ani-cli.git"
cp ani-cli/ani-cli "$PREFIX"/bin
rm -rf ani-cli
```

For players you can use the apk (playstore/fdroid) versions of mpv and vlc. Note that these cannot be checked from termux so a warning is generated when checking dependencies.

### iOS
Install iSH and VLC from the app store.

Make sure apk is updated using
```apk update; apk upgrade```
then run this:
```sh
apk add grep sed curl fzf git aria2 alpine-sdk ncurses
git clone https://github.com/Lockl00p/ffmpeglibs-iSH.git ~/ffmpeg
cd ~/ffmpeg
cat fmp.?? > ffmpeg.tar.gz
tar -xvf ffmpeg.tar.gz
cd FFmpeg
make install
cd
rm -rf ffmpeg
apk add ffmpeg
git clone https://github.com/pystardust/ani-cli ~/.ani-cli
cp ~/.ani-cli/ani-cli /usr/local/bin/ani-cli
chmod +x /usr/local/bin/ani-cli
rm -rf ~/.ani-cli
```
note that downloading is going to be very slow. This is an iSH issue, not an ani-cli issue.

### Steam Deck

#### Copypaste script:

* Switch to Desktop mode (`STEAM` Button > Power > Switch to Desktop)
* Open `Konsole` (Steam Deck Icon in bottom left corner > System > Konsole)
* Copy the script, paste it in the CLI and press Enter("A" button on Steam Deck)

```sh
[ ! -d ~/.local/bin ] && mkdir ~/.local/bin && echo "export $PATH=$HOME/.local/bin:$PATH" >> ".$(echo $SHELL | sed -nE "s|.*/(.*)\$|\1|p")rc"

git clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
~/.fzf/install

mkdir ~/.aria2c
curl -o ~/.aria2c/aria2-1.36.0.tar.bz2 https://github.com/q3aql/aria2-static-builds/releases/download/v1.36.0/aria2-1.36.0-linux-gnu-64bit-build1.tar.bz2
tar xvf ~/.aria2c/aria2-1.36.0.tar.bz2 -C ~/.aria2c/
cp ~/.aria2c/aria2-1.36.0-linux-gnu-64bit-build1/aria2c ~/.local/bin/
chmod +x ~/.local/bin/aria2c

curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o ~/.local/bin/yt-dlp
chmod +x ~/.local/bin/yt-dlp

git clone https://github.com/pystardust/ani-cli.git ~/.ani-cli
cp ~/.ani-cli/ani-cli ~/.local/bin/

flatpak install io.mpv.Mpv
```
press enter("A" button on Steam Deck) on questions

#### Installation in steps:

##### Install mpv (Flatpak version):

```sh
flatpak install io.mpv.Mpv
```
press enter("A" button on Steam Deck) on questions

##### Install [fzf](https://github.com/junegunn/fzf):

```sh
git clone --depth 1 https://github.com/junegunn/fzf.git ~/.fzf
~/.fzf/install
```
press enter("A" button on Steam Deck) on questions

##### Make a ~/.local/bin folder if doesn't exist and add it to $PATH

```sh
[ ! -d ~/.local/bin ] && mkdir ~/.local/bin && echo "export $PATH=$HOME/.local/bin:$PATH" >> ".$(echo $SHELL | sed -nE "s|.*/(.*)\$|\1|p")rc"
```

##### Install [aria2](https://github.com/aria2/aria2) (needed for download feature only):

```sh
mkdir ~/.aria2c
curl -o ~/.aria2c/aria2-1.36.0.tar.bz2 https://github.com/q3aql/aria2-static-builds/releases/download/v1.36.0/aria2-1.36.0-linux-gnu-64bit-build1.tar.bz2
tar xvf ~/.aria2c/aria2-1.36.0.tar.bz2 -C ~/.aria2c/
cp ~/.aria2c/aria2-1.36.0-linux-gnu-64bit-build1/aria2c ~/.local/bin/
chmod +x ~/.local/bin/aria2c
```

##### Install [yt-dlp](https://github.com/yt-dlp/yt-dlp) (needed for download feature only):

```sh
curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o ~/.local/bin/yt-dlp
chmod +x ~/.local/bin/yt-dlp
```

##### Install ani-cli:

```sh
git clone https://github.com/pystardust/ani-cli.git ~/.ani-cli
cp ~/.ani-cli/ani-cli ~/.local/bin/
```

##### Optional: add desktop entry:

```
echo '[Desktop Entry]
Encoding=UTF-8
Version=4.0
Type=Application
Exec=konsole -e ani-cli
Name=ani-cli' > ~/.local/share/applications/ani-cli.desktop
```
The .desktop entry will allow to start ani-cli in Konsole directly from "Gaming Mode"
In Steam Desktop app:
`Add game` > `Add a non-steam game` > tick a box for `ani-cli` > `Add selected programs`
*Note: Konsole window size bugs out if launched from "Gaming Mode".*
*Note: this is not working the way it should yet.*

## Uninstall

* apt:
```sh
sudo apt remove ani-cli
# to remove the repository from apt
sudo rm -f /etc/apt/trusted.gpg.d/ani-cli.asc /etc/apt/sources.list.d/ani-cli-debian.list
```
* dnf:
```sh
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
```sh
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
```sh
pkg remove ani-cli
```
* Android:
```sh
rm "$PREFIX/bin/ani-cli"
```
* Steam Deck
```sh
rm "~/.local/bin/ani-cli"
rm -rf ~/.ani-cli
```
optionally: remove dependencies:
```sh
rm ~/.local/bin/aria2c
rm ~/.local/bin/yt-dlp
rm -rf "~/.aria2"
rm -rf "~/.fzf"
flatpak uninstall io.mpv.Mpv
```
* iOS
```
rm -rf /usr/local/bin/ani-cli
```
To uninstall FFmpeg:
```
rm -rf /usr/local/lib/libavutil.a /usr/local/lib/libavcodec.a /usr/local/lib/libavformat.a /usr/local/lib/pkgconfig/libavutil.a /usr/local/lib/pkgconfig/libavcodec.a /usr/local/lib/pkgconfig/libavformat.a
apk del ffmpeg
```
To uninstall other dependencies:
```
apk del grep sed curl fzf git aria2 alpine-sdk ncurses
```
## Dependencies

- grep
- sed
- curl
- mpv - Video Player
- iina - mpv replacement for MacOS
- aria2c - Download manager
- yt-dlp - m3u8 Downloader
- ffmpeg - m3u8 Downloader (fallback)
- fzf - User interface

## Homies

* [animdl](https://github.com/justfoolingaround/animdl): Ridiculously efficient, fast and light-weight (supports most sources: allanime, zoro ... (Python)
* [jerry](https://github.com/justchokingaround/jerry): stream anime with anilist tracking and syncing, with discord presence (Shell)
* [anipy-cli](https://github.com/sdaqo/anipy-cli): ani-cli rewritten in python (Python)
* [saikou](https://github.com/saikou-app/saikou): Best android app for anime/manga with anilist integration (Kotlin)
* [mangal](https://github.com/metafates/mangal): Download & read manga from any source with anilist sync (Go)
* [lobster](https://github.com/justchokingaround/lobster): Watch movies and series from the terminal (Shell)
* [mov-cli](https://github.com/mov-cli/mov-cli): Watch movies/shows in the cli (Python/Shell)
* [dra-cla](https://github.com/CoolnsX/dra-cla): ani-cli equivalent for korean dramas (Shell)
* [redqu](https://github.com/port19x/redqu):  A media centric reddit client (Clojure)
