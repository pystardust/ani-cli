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

	# resume watching animeall: install

install:
	cp ani-cli /usr/bin

uninstall:
	rm -rf /usr/bin/ani-cli

Multiple episodes can be viewed/downloaded by giving the episode range like so

	Choose episode [1-13]: 1 6

This would open/download episodes 1 2 3 4 5 6

## Dependencies

* grep
* curl
* sed
* mpv
