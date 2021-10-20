all: install

install:
	mkdir $HOMEPATH/.cache > /dev/null
	cp ani-cli-win $WINDIR/system32/

uninstall:
	rm -rf $WINDIR/system32/ani-cli-win
