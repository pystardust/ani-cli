all: install

ifeq ($(OS), Windows_NT)
	Platform := "Windows"
else
        Platform = $(uname -o)
endif

install:
	ifeq ($(Platform), GNU/Linux)
		PREFIX := /usr/local
		cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
		chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	endif
	ifeq ($(Platform), Darwin)
		PREFIX := /usr/local
		cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
		chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	endif
	ifeq ($(Platform),Windows)
		mkdir $USERPROFILE/.cache 2> /dev/null
		cp ani-cli-win $WINDIR/system32/ani-cli
	endif
	ifeq ($(Platform),Android)
		TERMUX_BIN := /data/data/com.termux/files/usr/bin
		cp ani-cli $(TERMUX_BIN)/ani-cli
		chmod 0755 $(TERMUX_BIN)/ani-cli
		@echo 'am start --user 0 -a android.intent.action.VIEW -d "$$2" -e "http-header-fields" "$$1" -n is.xyz.mpv/.MPVActivity' > $(TERMUX_BIN)/mpv
		chmod +x $(TERMUX_BIN)/mpv
		mkdir -p $(HOME)/.cache
	endif

uninstall:
	ifeq ($(Platform), GNU/Linux)
		PREFIX := /usr/local
		rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
	endif
	ifeq ($(Platform), Darwin)
		PREFIX := /usr/local
		rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli
	endif
	ifeq ($(Platform),Windows)
		rm -rf $WINDIR/system32/ani-cli
	endif
	ifeq ($(Platform),Android)
		TERMUX_BIN := /data/data/com.termux/files/usr/bin
		rm -rf $(TERMUX_BIN)/ani-cli
		rm -rf $(TERMUX_BIN)/mpv
	endif

.PHONY: all install uninstall
