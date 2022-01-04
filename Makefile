PREFIX := /usr/local

#install by default
all: install

install:
	# copies ani-cli file to /usr/local/bin/ani-cli, which should be in path
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	# marks ani-cli executable
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli

uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/ani-cli

.PHONY: all install uninstall
