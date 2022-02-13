
PREFIX ?= /usr/local

all:
		@echo plz type \'sudo make install\' to install ani-cli

install:
		@install -Dm755 ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
		@echo "successfully installed ani-cli"

uninstall:
		@rm -f $(DESTDIR)$(PREFIX)/bin/ani-cli
		@echo "successfully removed ani-cli"
