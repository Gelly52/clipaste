PREFIX ?= /usr/local

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	install -d $(PREFIX)/bin
	install -m 755 target/release/clipaste $(PREFIX)/bin/clipaste

uninstall:
	rm -f $(PREFIX)/bin/clipaste

clean:
	cargo clean
