# Contributing to the Code Base

## Building

To build, you will need a compatible version of the Rust toolchain installed.  Please follow the [Rust installation instructions](https://rust-lang.org/tools/install/) for getting your environment prepared.

You may also find the supplied [`Makefile`](Makefile) valuable in running common tasks, which requires having a compatible make tool also available.  This isn't required, though.


## Testing

The tool includes a collection of integration tests that require first building the integration test executables.  These allow for checking whether the sandbox techniques allow for the expected behavior, either restricting access or allowing it.

To build them, you will need to run `cargo build` inside each child directory of [`tests`](tests).  Alternatively, if you have a make tool installed, you can run `make test-bin` to compile them all.  This will build the debug version of them.

Once you have these built, you can run `cargo test` from the root directory to execute the unit and integration tests.


## Submitting a Contribution

All contributions must be submitted under the [project's license](LICENSE).

### Submitting a Code PR

*TBD*

### Submitting a Documentation PR

*TBD*
