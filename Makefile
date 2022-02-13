
PREFIX ?= /usr/local

all:
		@echo RUN \'sudo make install\' to install ani-cli

install:
		@install -Dm755 ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
		@echo "ani-cli installed"

uninstall:
		@rm -f $(DESTDIR)$(PREFIX)/bin/ani-cli
		@echo "ani-cli removed"
