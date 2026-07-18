<h3 align="center">
A cli to browse and watch anime (alone AND with friends).
</h3>

<h4 align="center">
Fork of <a href="https://github.com/pystardust/ani-cli">pystardust/ani-cli</a> with:
<br><br>
&#x2022; <a href="https://anidb.app">anidb.app</a> as primary source (direct HLS streams, no Cloudflare)<br>
&#x2022; allanime as fallback (AES-256-GCM crypto, no botan dependency)<br>
&#x2022; Wider anime coverage — anime missing from allanime now playable
</h4>

## Table of Contents

- [Install](#install)
- [Update](#update)
- [Uninstall](#uninstall)
- [Dependencies](#dependencies)
- [FAQ](#faq)

## Install

### Quick install (curl)

```sh
curl -sL https://raw.githubusercontent.com/VVAT3R/ani-cli/master/install.sh | sudo sh
```

### Manual install

Install dependencies (see [below](#dependencies)), then:

```sh
git clone "https://github.com/VVAT3R/ani-cli.git"
sudo cp ani-cli/ani-cli /usr/local/bin
sudo cp ani-cli/aesgcm /usr/local/bin
sudo cp ani-cli/aesgcm.c /usr/local/bin
rm -rf ani-cli
```

## Update

```sh
sudo ani-cli -U
```

## Uninstall

```sh
curl -sL https://raw.githubusercontent.com/VVAT3R/ani-cli/master/uninstall.sh | sudo sh
```

## Dependencies

- `curl` — HTTP requests
- `sed`, `grep` — text processing
- `openssl` — crypto operations (on Termux: `pkg install openssl-tool`)
- `gcc` — compiles aesgcm helper on first run
- `mpv` — video player (or `iina` on macOS, `vlc` with `--vlc`)
- `fzf` — interactive menu
- `ffmpeg` — download support (with `-d`)
- `aria2c` — download support (with `-d`)
- `yt-dlp` — download support (with `-d`)
- `ani-skip` *(optional)* — auto-skip intros

## FAQ

- **Can I watch dub?** — Yes, `ani-cli --dub`
- **Can I change quality?** — Yes, `ani-cli -q 1080`
- **Can I download?** — Yes, `ani-cli -d`
- **Can I use vlc?** — Yes, `ani-cli --vlc`
- **How do I bulk download?** — `ani-cli -d -e 1-100 one piece`
- **Full options** — `ani-cli --help`
