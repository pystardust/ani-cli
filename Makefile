DIR := /usr/local/#$${HOME}/.local/
BIN := bin/

.PHONY: install
install:
	chmod +x ./ani-cli
	cp ./ani-cli "$(DIR)$(BIN)"

.PHONY: update
update:
	git pull
	chmod +x ./ani-cli
	rm -f "$(DIR)$(BIN)ani-cli"
	cp ./ani-cli "$(DIR)$(BIN)"

.PHONY: uninstall
uninstall:
	rm -f "$(DIR)$(BIN)ani-cli"
