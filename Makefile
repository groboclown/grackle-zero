

CARGO := cargo

DEBUG_TARGET := target/debug/libgrackle_zero.d
SRC_FILES := $(shell find src -name '*.rs')


build: test build-default

clean: .FORCE
	$(CARGO) clean
	$(MAKE) -C tests clean

test: test-bin .FORCE
	$(CARGO) test

test-bin: .FORCE
	$(MAKE) -C tests

$(DEBUG_TARGET): $(SRC_FILES)
	$(CARGO) build

build-default: $(DEBUG_TARGET)

build-linux: .FORCE
	cargo build --target x86_64-unknown-linux-gnu

build-win: .FORCE
	# If running from Linux, this requires installing mingw-w64-gcc
	cargo build --target x86_64-pc-windows-gnu


.FORCE:
