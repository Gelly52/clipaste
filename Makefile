PREFIX ?= /usr/local

UNAME_S := $(shell uname -s)

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	install -d $(PREFIX)/bin
	install -m 755 target/release/clipaste $(PREFIX)/bin/clipaste
ifeq ($(UNAME_S),Darwin)
	codesign --force --sign - $(PREFIX)/bin/clipaste
endif

uninstall:
	rm -f $(PREFIX)/bin/clipaste

clean:
	cargo clean
