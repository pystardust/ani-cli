# Use ani-cli for Android devices (through Termux)

A cli to browse and watch anime.

This tool scrapes the site 9anim.vip

## Setup

### Downloading necessary apps

* Download Termux from [F-Droid](https://f-droid.org/en/packages/com.termux/).

* Download mpv-android from [Play Store](https://play.google.com/store/apps/details?id=is.xyz.mpv) or [F-Droid](https://f-droid.org/packages/is.xyz.mpv). (VLC is also an option, however, it does not work well. Video stops working after few seconds).

### Setup in termux

* Open Termux and run:
```
pkg install git curl mpv 
```

* Run this command to install youtube-dl on Termux:
```
termux-setup-storage ; curl -s -L https://raw.githubusercontent.com/shukryshuk/androidydl/master/youtubedl.sh | bash
```
`termux-setup-storage` asks for storage permissions so that Termux can access the shared storage in your device. 

* Use the `git clone` command to clone this repository on your device
```
git clone https://github.com/nasseef20/ani-cli-android.git
```

## Usage

```
cd ani-cli
./ani-cli <Enter search query. Use quotation marks if its more than a word>
```



## Dependencies

* curl
* mpv
* youtube-dl



