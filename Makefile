all: install

ifeq ($(OS), Windows_NT)
	Platform := "Windows"
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
else ifeq ($(Platform), Darwin)
	PREFIX := /usr/local
install:
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
else ifeq ($(Platform),Windows)
install:
	mkdir $USERPROFILE/.cache 2> /dev/null
	cp ani-cli-win $WINDIR/system32/ani-cli
uninstall:
	rm -rf $WINDIR/system32/ani-cli
else ifeq ($(Platform),Android)
install:
	TERMUX_BIN := /data/data/com.termux/files/usr/bin
	cp ani-cli $(TERMUX_BIN)/ani-cli
	chmod 0755 $(TERMUX_BIN)/ani-cli
	@echo 'am start --user 0 -a android.intent.action.VIEW -d "$$2" -e "http-header-fields" "$$1" -n is.xyz.mpv/.MPVActivity' > $(TERMUX_BIN)/mpv
	chmod +x $(TERMUX_BIN)/mpv
	mkdir -p $(HOME)/.cache
uninstall:
	TERMUX_BIN := /data/data/com.termux/files/usr/bin
	rm -rf $(TERMUX_BIN)/ani-cli
	rm -rf $(TERMUX_BIN)/mp
else
install:
	@echo 'Failed to detect your operating system'
uninstall:
	@echo 'Failed to detect your operating system'
endif

.PHONY: all install uninstall
