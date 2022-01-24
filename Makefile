all: install

ifeq ($(OS), Windows_NT)
install:
	mkdir $(USERPROFILE)\.cache 2> /dev/null
	cp ani-cli $(WINDIR)/system32/ani-cli
	echo "Installation successful (Windows)"
uninstall:
	rm -rf $WINDIR/system32/ani-cli
	echo "Removal successful (Windows)"
else
        Platform = $(shell uname -o)
endif

ifeq ($(Platform), GNU/Linux)
	PREFIX := /usr/local
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Installation successful (Linux)"
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Removal successful (Linux)"
else ifeq ($(Platform), Darwin)
	PREFIX := /usr/local
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Installation successful (Mac OS)"
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Removal successful (Mac OS)"
else ifeq ($(Platform), Android)
	TERMUX_BIN := /data/data/com.termux/files/usr/bin
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Installation successful (Android Termux)"
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
	echo "Removal successful (Android Termux)"
else
install:
	echo "Failed to detect Platform"
uninstall:
	echo "Failed to detect Platform"
endif

.SILENT .PHONY: all install uninstall
