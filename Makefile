PREFIX := /usr/local

# Specify path of install directory
# The install directory contains a .git file, which is used on startup to determine if an update is available
CURRENT_DIR := '$(shell pwd)'

all: install

install:
	# writes ORIGINAL_DIR variable to first line of ani-cli file
	sed -i '2s}.*}ORIGINAL_DIR=$(CURRENT_DIR)}' ./ani-cli
	cp ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli

uninstall:
	$(RM) $(DESTDIR)$(PREFIX)/bin/ani-cli

.PHONY: all install uninstall
