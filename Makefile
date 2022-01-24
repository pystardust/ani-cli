all: install

ifeq ($(OS), Windows_NT)
	Platform=Msys
install:
	mkdir $(USERPROFILE)\.cache 2> /dev/null
	cp ani-cli $(WINDIR)/system32/ani-cli
uninstall:
	rm -rf $WINDIR/system32/ani-cli
else
        Platform = $(shell uname -o)
endif

ifeq ($(Platform), GNU/Linux)
	PREFIX := /usr/local
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
else
ifeq ($(Platform), Darwin)
	PREFIX := /usr/local
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
else
ifeq ($(Platform), Android)
	TERMUX_BIN := /data/data/com.termux/files/usr/bin
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
endif
endif
endif

.PHONY: all install uninstall
