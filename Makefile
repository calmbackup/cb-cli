BINARY_NAME=calmbackup
VERSION?=dev

.PHONY: build test release clean

build:
	cargo build --release
	mkdir -p bin
	cp target/release/$(BINARY_NAME) bin/$(BINARY_NAME)

test:
	cargo test

release: release-linux-amd64 release-linux-arm64

release-linux-amd64:
	cargo build --release --target x86_64-unknown-linux-gnu
	mkdir -p dist
	tar -czf dist/$(BINARY_NAME)_$(VERSION)_linux_amd64.tar.gz \
		-C target/x86_64-unknown-linux-gnu/release $(BINARY_NAME)

release-linux-arm64:
	cross build --release --target aarch64-unknown-linux-gnu
	mkdir -p dist
	tar -czf dist/$(BINARY_NAME)_$(VERSION)_linux_arm64.tar.gz \
		-C target/aarch64-unknown-linux-gnu/release $(BINARY_NAME)

clean:
	cargo clean
	rm -rf bin/ dist/
