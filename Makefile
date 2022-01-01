PREFIX := /usr/local

# Specify path of install directory
# The install directory contains a .git file, which is used on startup to determine if an update is available
CURRENT_DIR := $(shell pwd)

all: install

install:
	# writes ORIGINAL_DIR variable to ani-cli file
	sed -i '2s}.*}ORIGINAL_DIR="$(CURRENT_DIR)"}' ./ani-cli
  # symlinks ani-cli file to /usr/local/bin/ani-cli, which should be in path
	ln -sr ani-cli $(DESTDIR)$(PREFIX)/bin/ani-cli
  # marks ani-cli symlink executable
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli

uninstall:
  # removes symlink in /usr/local/bin/ani-cli
	$(RM) $(DESTDIR)$(PREFIX)/bin/ani-cli

.PHONY: all install uninstall
