# ani-cli-win

> Script working again :), thanks to fork by
> [Dink4n](https://github.com/Dink4n/ani-cli) for the alternative approach to
> by pass the captcha on [gogoanime](https://gogoanime.vc)

A cli to browse and watch anime.

This tool scrapes the site [gogoanime](https://gogoanime.pe).


## Download
```bash
git clone https://github.com/pystardust/ani-cli.git
```

## Install
1- Make Sure you have git bash to run this on windows [GitBash](https://git-scm.com/downloads).

2- Install Vlc [vlc](https://www.videolan.org/).

3- Add vlc to Windows Env PATH :  example : C:\Program Files\VideoLAN\VLC.

4- Open git bash and do the following commands : 

```bash
cd ani-cli
git checkout windows-vlc
chmod +x ani-cli-win
sudo make
```

## Usage

	# watch anime
	ani-cli <query>

	# download anime
	ani-cli -d <query>

	# resume watching anime
	ani-cli -H

	# set video quality
	ani-cli -q 360

By default `ani-cli` would try to get the best video quality available  
You can give specific qualities like `360/480/720/..`

You can also use special names:

* `best`: Select the best quality available
* `worst`: Select the worst quality available


Multiple episodes can be viewed/downloaded by giving the episode range like so

	Choose episode [1-13]: 1 6

This would open/download episodes 1 2 3 4 5 6

## Dependencies

* grep
* curl
* sed
* vlc
