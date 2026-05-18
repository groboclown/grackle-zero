# Simple cross-compiler Makefile for simple-cfg
#
# Supports:
#   - gcc
#   - clang
#   - Microsoft cl
#   - MinGW
#
# Examples:
#   make
#   make CC=clang
#   make CC=cl
#
# Notes:
#   - outputs into target/debug to match up with what the Rust test-bin projects do.

PROGRAM = simple-cfg
SRC = main.c
MAKEFILE_DIR = $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# Detect platform.
ifeq ($(OS),Windows_NT)
  # Windows
  EXE = .exe
  VERIFY = @powershell -NoProfile -ExecutionPolicy Bypass -File "verify.ps1" "$(TARGET)"
  RUN_TARGET = @powershell -NoProfile -ExecutionPolicy Bypass -Command "& '$(TARGET)' ; exit \$$LASTEXITCODE"

  ifeq ($(CC),cl)
    # Microsoft compiler.
    OUTDIR = target\debug
    MKDIR_P = @powershell -NoProfile -ExecutionPolicy Bypass -Command "& md '$(OUTDIR)' -ea 0" ; exit 0
    RM = del /Q
    OBJ = .obj
    CFLAGS ?= /nologo /W4 /O2 /GS- /guard:cf,longjmp
    LDFLAGS ?= /link /nologo /subsystem:console /entry:entry /nodefaultlib kernel32.lib
    COMPILE = $(CC) $(CFLAGS) /c /Fo$@ $(SRC)
    LINK = $(CC) /Fe$@ $^ $(LDFLAGS)

  else
    OUTDIR = target/debug
    MKDIR_P = mkdir -p $(OUTDIR)
    RM = rm -f
    OBJ = .o

    ifeq ($(CC),clang)
      # clang on Windows drives lld-link here, so use MSVC-style link flags.
      # Keep compile flags freestanding and rely on linker CFG options.
      CFLAGS ?= -O2 -Wall -Wextra -ffreestanding
      LDFLAGS ?= -nostdlib -fuse-ld=lld -Wl,/subsystem:console -Wl,/entry:entry -Wl,/guard:cf -lkernel32
    else
      CFLAGS ?= -O2 -Wall -Wextra -ffreestanding
      LDFLAGS ?= -nostdlib -fuse-ld=lld -Wl,--subsystem,console -Wl,--entry=entry -Wl,--guard-cf -Wl,--no-guard-longjmp -lkernel32
    endif
    COMPILE = $(CC) $(CFLAGS) -c -o $@ $(SRC)
    LINK = $(CC) -o $@ $^ $(LDFLAGS)
  endif

else
  # MacOS or *nix.
  # For these platforms, this binary has no point.
  COMPILE = echo "Skipping $(PROGRAM)"
  LINK = echo "Skipping $(PROGRAM)"
  VERIFY = echo "Skipping verify"
  RUN_TARGET = echo "Skipping run"
  MKDIR_P = echo "Skipping mkdir"
endif

TARGET = $(OUTDIR)/$(PROGRAM)$(EXE)
OBJECTS = $(OUTDIR)/main$(OBJ)

$(TARGET): $(OBJECTS)
	@echo "Linking"
	$(LINK)

$(OUTDIR)/main$(OBJ): $(SRC)
	$(MKDIR_P)
	@echo "Compiling"
	$(COMPILE)

clean:
	-$(RM) $(TARGET) $(OBJECTS)

verify: $(TARGET)
	$(VERIFY)
	$(RUN_TARGET)

all: clean $(TARGET) verify

.PHONY: all clean verify
