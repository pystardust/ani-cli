# ani-cli

> Use the version from [PR 209](https://github.com/RaynardGerraldo/ani-cli) for now. 
> 
> The PR is planned to be merged this week.

> Another crippling issue is [Issue 202](https://github.com/pystardust/ani-cli/issues/202)
> 
> This makes some series not work while others do.
> 
> It is the next big task after 209 is merged.
> 
> Take a look and tell us if you want to start working on a solution, please mention it on the issue.

A cli to browse and watch anime.

This tool scrapes the site [gogoanime](https://gogoanime.pe).

## Download

```bash
git clone https://github.com/pystardust/ani-cli.git
```

## Install

```bash
cd ani-cli
sudo make
```

## Usage

  ### watch anime
  ``ani-cli <query>``

  ### download anime
  ``ani-cli -d <query>``

  ### resume watching anime
  ``ani-cli -H``

  ### delete anime from history
  ``ani-cli -D``

  ### set video quality
  ``ani-cli -q 360``

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
* mpv
* ffmpeg

## Misc

- Windows instructions can be found in this branch https://github.com/pystardust/ani-cli/tree/windows-vlc
