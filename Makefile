PREFIX := /usr/local

# Specify path of install directory
# The install directory contains a .git file, which is used on startup to determine if an update is available
CURRENT_DIR := '$(shell pwd)'

all: compile install run

compile:
	shards build --release --production --verbose -p -t

install:
	chmod +x ./bin/ani-cli
	mv ./bin/ani-cli /usr/bin

run:
	ani-cli

.PHONY: all compile install run