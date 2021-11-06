all: install

install:
	cp ani-cli /usr/bin
	npm install --global ffmpeg-progressbar-cli
uninstall:
	rm -rf /usr/bin/ani-cli
