TERMUX_BIN := /data/data/com.termux/files/usr/bin
#install by default
all: install

install:
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

uninstall:
	rm -rf $(TERMUX_BIN)/ani-cli
	rm -rf $(TERMUX_BIN)/mpv
	
.PHONY: all install uninstall
