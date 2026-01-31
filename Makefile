

CARGO := cargo

DEBUG_TARGET := target/debug/libgrackle_zero.d
SRC_FILES := $(shell find src -name '*.rs')



build: test $(DEBUG_TARGET)

test: test-bin .FORCE
	$(CARGO) test

test-bin: .FORCE
	$(MAKE) -C tests

$(DEBUG_TARGET): $(SRC_FILES)
	$(CARGO) build


.FORCE:
