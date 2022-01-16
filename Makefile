#install by default
all: install

install:
	# copies ani-cli file to /data/data/com.termux/usr/bin/ani-cli, which should be in path
	cp ani-cli $(PREFIX)/bin/ani-cli
	# marks ani-cli executable
	chmod 0755 $(PREFIX)/bin/ani-cli
	# copies mpv file fo /data/data/com.termux/usr/bin/mpv, which shoud be in path
	cp mpv $(PREFIX)/bin/mpv
	# marks mpv executable
	chmod +x $(PREFIX)/bin/mpv
	# creating .cache folder
	mkdir $(HOME)/.cache

uninstall:
	rm -rf $(PREFIX)/bin/ani-cli
	rm -rf $(PREFIX)/bin/mpv

.PHONY: all install uninstall
