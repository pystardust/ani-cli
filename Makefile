PREFIX := /usr/local

all: install

install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	npm install --global ffmpeg-progressbar-cli
uninstall:
	$(RM) $(DESTDIR)$(PREFIX)/bin/ani-cli

.PHONY: all install uninstall
