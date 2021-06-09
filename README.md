# ani-cli

A cli to browse and watch anime.

This tool scrapes the site [gogoanime](https://gogoanime.vc).

If certain episode is downloadable then mpv would be used to play that episode.
If downloadable link not available the user would be prompted to open the
stream in browser.

## Usage

	ani-cli <query>

## Dependencies

* curl
* sed
* mpv
