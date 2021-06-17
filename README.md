# ani-cli

A cli to browse and watch anime.

This tool scrapes the site [gogoanime](https://gogoanime.vc).


## Usage

	# watch anime
	ani-cli <query>

	# download anime
	ani-cli -d <query>

	# chose from history 
	ani-cli -H <query>

Multiple episodes can be viewed/downloaded by giving the episode range like so

	Choose episode [1-13]: 1 6

This would open/download episodes 1 2 3 4 5 6

## Dependencies

* curl
* sed
* grep
* mpv
