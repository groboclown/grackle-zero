# Simple cross-compiler Makefile for simple-c
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

PROGRAM = simple-c
SRC = main.c

# Detect platform.
ifeq ($(OS),Windows_NT)
  # Windows
  EXE = .exe

  ifeq ($(CC),cl)
    # Microsoft compiler.
    OUTDIR = target\debug
    MKDIR_P = if not exist $(OUTDIR) mkdir $(OUTDIR)
    RM = del /Q
    OBJ = .obj
    CFLAGS ?= /nologo /W4 /O2 /GS-
    LDFLAGS ?= /link /nologo /subsystem:console /entry:entry /nodefaultlib kernel32.lib
    COMPILE = $(CC) $(CFLAGS) /c /Fo$@ $(SRC)
    LINK = $(CC) /Fe$@ $^ $(LDFLAGS)

  else
    # Non-cl Windows toolchains.
    OUTDIR = target/debug
    MKDIR_P = mkdir -p $(OUTDIR)
    RM = rm -f
    OBJ = .o
    CFLAGS ?= -O2 -Wall -Wextra -ffreestanding

    ifeq ($(CC),clang)
      # clang targeting the MSVC environment uses lld-link/link semantics.
      LDFLAGS ?= -nostdlib -Wl,/subsystem:console -Wl,/entry:entry -lkernel32
    else
      # MinGW/GNU ld semantics.
      LDFLAGS ?= -nostdlib -Wl,--subsystem,console -Wl,--entry,entry -lkernel32
    endif

    COMPILE = $(CC) $(CFLAGS) -c -o $@ $(SRC)
    LINK = $(CC) -o $@ $^ $(LDFLAGS)
  endif

else
  # MacOS or *nix.
  # Creates a static (no .so dependency) executable, where possible.
  EXE =
  OUTDIR = target/debug
  OBJ = .o
  CFLAGS ?= -O2 -Wall -Wextra -pedantic
  ifneq ($(UNAME_S),Darwin)
  	LDFLAGS ?= -static
  else
    LDFLAGS ?=
  endif
  RM = rm -f
  MKDIR_P = mkdir -p $(OUTDIR)
  COMPILE = $(CC) $(CFLAGS) -c -o $@ $(SRC)
  LINK = $(CC) -o $@ $^ $(LDFLAGS)
endif

TARGET = $(OUTDIR)/$(PROGRAM)$(EXE)
OBJECTS = $(OUTDIR)/main$(OBJ)

all: $(TARGET)

$(TARGET): $(OBJECTS)
	$(LINK)

$(OUTDIR)/main$(OBJ): $(SRC) $(OUTDIR)
	$(COMPILE)

$(OUTDIR):
	$(MKDIR_P)

clean:
	-$(RM) $(TARGET) $(OBJECTS)

.PHONY: all clean
