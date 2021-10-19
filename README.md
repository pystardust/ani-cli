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

## Dependencies

* grep
* curl
* sed
* mpv

## Windows install instructions
1.  Install git bash from https://gitforwindows.org/ It has all the depedencies except mpv.
2. Install scoop by following the instructions in https://scoop.sh/ , the Powershell instructions from the website are listed below.
	```bash
	Set-ExecutionPolicy RemoteSigned -scope CurrentUser
	iwr -useb get.scoop.sh | iex
	Set-ExecutionPolicy Restricted -scope CurrentUser
	```
3. After installing scoop, run the below commands in git-bash.
	```bash
	scoop bucket add extras
	scoop install mpv
	```
4. Go to ani-cli folder, open git-bash in that location and  run the below to start the script.
	```bash
	./ani-cli
	```