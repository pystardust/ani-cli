PREFIX := /usr/local

# Specify path of install directory
# The install directory contains a .git file, which is used on startup to determine if an update is available
CURRENT_DIR := $(shell pwd)
UNAME := $(shell uname)
ifeq ($(UNAME), Darwin)
	SED_FLAGS := "-i ''"
	LN_FLAGS := "-s"
	LN_SRC := "$(CURRENT_DIR)/ani-cli"
else ifeq ($(UNAME), Linux)
	SED_FLAGS := "-i"
	LN_FLAGS := "-sr"
	LN_SRC := "ani-cli"
endif

all: install

install: uninstall
	@# writes ORIGINAL_DIR variable to ani-cli file
	sed $(SED_FLAGS) '2s}.*}ORIGINAL_DIR="$(CURRENT_DIR)"}' ./ani-cli
	@# symlinks ani-cli file to /usr/local/bin/ani-cli, which should be in path
	ln $(LN_FLAGS) $(LN_SRC) $(DESTDIR)$(PREFIX)/bin/ani-cli
	@# marks ani-cli symlink executable
	chmod 0755 $(DESTDIR)$(PREFIX)/bin/ani-cli

uninstall:
	@# removes symlink in /usr/local/bin/ani-cli
	$(RM) $(DESTDIR)$(PREFIX)/bin/ani-cli

.PHONY: all install uninstall
