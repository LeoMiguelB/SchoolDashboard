.PHONY: all build release install uninstall test clean

all: release

build:
	cargo build

release:
	cargo build --release

install: release
	mkdir -p $(HOME)/.local/bin
	cp -f target/release/schoodash $(HOME)/.local/bin/schoodash
	chmod +x $(HOME)/.local/bin/schoodash
	ln -sf $(HOME)/.local/bin/schoodash $(HOME)/.local/bin/schooldash
	@echo "✔ schoodash installed to $(HOME)/.local/bin/schoodash"

uninstall:
	rm -f $(HOME)/.local/bin/schoodash
	rm -f $(HOME)/.local/bin/schooldash
	@echo "✔ schoodash uninstalled from $(HOME)/.local/bin"

test:
	cargo test

clean:
	cargo clean
