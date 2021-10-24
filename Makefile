all: install

install:
	mkdir $USERPROFILE/.cache 2> /dev/null
	cp ani-cli-win $WINDIR/system32/ani-cli

uninstall:
	rm -rf $WINDIR/system32/ani-cli
