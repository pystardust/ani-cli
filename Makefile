all: install

install:
	cp ani-cli-win %windir%/system32/

uninstall:
	rm -rf %windir%/system32/ani-cli-win