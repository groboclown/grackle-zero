
CARGO := cargo

SRC_FILES := $(wildcard src/*.rs)


build: $(SRC_FILES)
	$(CARGO) build

clean: .FORCE
	$(CARGO) clean

format: $(SRC_FILES)
	$(CARGO) fmt


.FORCE:
