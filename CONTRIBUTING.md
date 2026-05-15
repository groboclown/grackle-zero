# Contributing to the Code Base

## Building

### Dependencies

To build, you will need a compatible version of the Rust toolchain installed.  Please follow the [Rust installation instructions](https://rust-lang.org/tools/install/) for getting your environment prepared.

You may also find the supplied [`Makefile`](Makefile) valuable in running common tasks, which requires having a compatible make tool also available.  This isn't required, though.

Building for Linux requires:

* Install the libseccomp library and its development headers.  For Ubuntu and Debian distributions, this looks like `apt-get install libseccomp-dev`

Windows build requirements:

* Either the MSVC toolchain or Clang MinGW toolchain.


## Testing

### Unit Tests

The tool includes a collection of integration tests that require first building the integration test executables.  These allow for checking whether the sandbox techniques allow for the expected behavior, either restricting access or allowing it.

To build them, you will need to run `cargo build` inside each child directory of [`test-bin`](test-bin).  Alternatively, if you have a make tool installed, you can run `make test-bin` to compile them all.  This will build the debug version of them.

Once you have these built, you can run `cargo test` from the root directory to execute the unit and integration tests.

The [`test-bin/simple-c`](test-bin/simple-c) and [`test-bin/simple-cfg`](test-bin/simple-cfg) directories, instead, use a C program so that tests can check for a very explicitly crafted executable - specifically, for Windows testing.  For Windows, they build without the C-like interface, in order to only have dependencies on `kernel32.dll`.  For `simple-cfg`, it also adds in Control Flow Guard protections.  Due to limitations in the Gnu MinGW toolchain, these require you to have either the MSVC tools or the Clang MinGW toolchain installed to build correctly.  If you have both MinGW GNUC and Clang, then you'll need to explicitly build with `make CC=clang`.


## Submitting a Contribution

All contributions must be submitted under the [project's license](LICENSE).

### Submitting a Code PR

*TBD*

### Submitting a Documentation PR

*TBD*

### Submitting an Issue

*TBD*

### Submitting a Security Discovery

As the primary focus for this project involves constructing a secure environment, the team takes all security issues seriously.  Please read through the [security information](SECURITY.md) document for how to go about submitting discoveries.
