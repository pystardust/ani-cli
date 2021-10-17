# ani-cli

> Script working again :), thanks to fork by
> [Dink4n](https://github.com/Dink4n/ani-cli) for the alternative approach to
> by pass the captcha on [gogoanime](https://gogoanime.vc)

A cli to browse and watch anime.

This tool scrapes the site [gogoanime](https://gogoanime.vc).

## Download

```bash
git clone https://github.com/pystardust/ani-cli.git
```

## Install

```bash
cd ani-cli
chmod +x ani-cli
sudo make
```

## Usage

    # watch anime
    ani-cli <query>

    # download anime
    ani-cli -d <query>

    # resume watching anime
    ani-cli -H

Multiple episodes can be viewed/downloaded by giving the episode range like so

    Choose episode [1-13]: 1 6

This would open/download episodes 1 2 3 4 5 6

## How to use in Windows OS:

    You'll need an extra 5 minutes if you are on windows.
    # Step-1 : You need to download cygwin (link below)
    # Step-2 : Make sure to select "curl" in the cygwin installion menu, this will download curl automatically.
    # Step-3 : Now you need to download mpv, to play the video. (link below)
    # Step-4 : After unzipping mpv, add it to your environment path.
    # Step-5 : Open the file "ani-cli" in this repo using an editor (vim/nano)
    # Step-6 : Change the first line, player_fn="mpv" to player_fn="mpv.com"
    # Step-7 : That's it, exit the editor and run the file inside cygwin using ./ani-cli

    Cygwin : https://www.cygwin.com/setup-x86_64.exe
    mpv    : https://sourceforge.net/projects/mpv-player-windows/files/64bit/mpv-x86_64-20211010-git-b3f3c3f.7z/download

## Dependencies

- grep
- curl
- sed
- mpv
