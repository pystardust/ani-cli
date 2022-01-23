#install by default
all: install

Platform = $(uname)

install:
	ifeq($(Platform), Linux)
		PREFIX := /usr/local
		# copies ani-cli file to /usr/local/bin/ani-cli, which should be in path
		cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
		# marks ani-cli executable
		chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli
	endif
	ifeq ($(Platform),Windows)
		mkdir $USERPROFILE/.cache 2> /dev/null
		cp ani-cli-win $WINDIR/system32/ani-cli
	endif
	ifeq ($(Platform),Android)
		TERMUX_BIN := /data/data/com.termux/files/usr/bin
		# copies ani-cli file to /data/data/com.termux/usr/bin/ani-cli, which should be in path
		cp ani-cli $(TERMUX_BIN)/ani-cli
		# marks ani-cli executable
		chmod 0755 $(TERMUX_BIN)/ani-cli
		# creating mpv file
		@echo 'am start --user 0 -a android.intent.action.VIEW -d "$$2" -e "http-header-fields" "$$1" -n is.xyz.mpv/.MPVActivity' > $(TERMUX_BIN)/mpv
		# marks mpv executable
		chmod +x $(TERMUX_BIN)/mpv
		# creating .cache folder
		mkdir -p $(HOME)/.cache
	endif

uninstall:
	ifeq($(Platform), Linux)
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
