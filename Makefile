

CARGO := cargo
RUSTUP := rustup

WINDOWS_TARGET := x86_64-pc-windows-gnu
LINUX_TARGET := x86_64-unknown-linux-gnu

# Recursive wildcard find.  Works in Windows and Unix variants.
# See https://stackoverflow.com/questions/2483182/recursive-wildcards-in-gnu-make/18258352#18258352
# See https://blog.jgc.org/2011/07/gnu-make-recursive-wildcard-function.html
rwildcard=$(foreach d,$(wildcard $(1:=/*)),$(call rwildcard,$d,$2) $(filter $(subst *,%,$2),$d))

SRC_FILES := $(call rwildcard,src,*.rs)

##         build: Standard test+compile target.
build: test build-default

##        format: Run the formatter on the source files.
##                This runs across the whole project.
format: $(SRC_FILES)
	$(CARGO) fmt
	$(MAKE) -C test-bin format

##         clean: Clean all build artifacts.
clean: .FORCE
	$(CARGO) clean
	$(MAKE) -C test-bin clean

##          test: Run all tests.
##                This will also compile the binaries required by the tests.
test: test-bin .FORCE
	$(CARGO) test

##      test-bin: Compile the test binaries.
##                These binaries are used by the unit tests to ensure the
##                sandbox correctly limits the execution abilities.
test-bin: .FORCE
	$(MAKE) -C test-bin

## build-default: Compile for your current default platform.
build-default: $(SRC_FILES)
	$(CARGO) build

##   build-linux: Compile for Linux.
build-linux: .FORCE
	cargo build --target $(LINUX_TARGET)

##     build-win: Compile for Windows.
build-win: .FORCE
	# If running from Linux, this requires installing mingw-w64-gcc
	cargo build --target $(WINDOWS_TARGET)

##  dependencies: Install all tool dependencies.
##                These are all the dependencies used by this and the
##                sub-projects.
dependencies: .FORCE
	$(RUSTUP) component add rustfmt
	$(RUSTUP) target add $(WINDOWS_TARGET)
	$(RUSTUP) target add $(LINUX_TARGET)


.FORCE:
