# Project Grackle Zero

Execute Tasks in Zero Access Mode.

## Purpose

This runs executions, either for the current program or other programs, in a zero-access sandbox.  It only communicates to the parent process through the file descriptors constructed during setup.

## Use

The library provides a basic framework for building a communication pattern, which you can build upon to monitor and interact with the child process.

Here's a trivial example of launching a program within a sandbox, then interacting with it by sending data to its `stdin`, and reading from its `stdout`.  It intentionally leaves its `stderr` untouched, so that any message sent to the child program's `stderr` is also output through the parent program's `stderr`.

```rust
use gracklezero::{sandbox_child, CommHandler, LaunchEnv};
use std::io::{Read, Write};
use std::ffi::OsString;
use std::path::PathBuf;

struct Handler {}

impl CommHandler for Handler {
  fn handle(self, mut child: Box<dyn spawn::Child>) -> Result<(), std::io::Error> {
    let mut send = child.take_stream_to_child(0).expect("no stdin");
    let mut recv = child.take_stream_from_child(1).expect("no stdout");
    send.write_all(b"ACK")?;
    drop(send);
    let mut received = String::new();
    recv.read_to_end(&mut received)?;
    println!("Received: {}", received);
    match child.
    Ok(())
  }
}

fn main() {
  let cmd = CommHandler{};
  let exit_code = sandbox_child(
      LaunchEnv {
          cmd: PathBuf::from("the-child-to-sandbox"),
          args: vec![OsString::from("an-argument")],
          cwd: PathBuf::from("."),
          env: std::collections::HashMap::new(),
          // Use stdin to send data to the child process,
          //     stdout to receive data from the child process,
          //     leave stderr untouched for error reporting through the parent process's stderr.
          fds: FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild]),
      },
      handler,
  ).expect("the sandbox execution should not cause an error");
  println!("Child exited with {}", exit_code);
}
```

## Communication Protocol

To have a useful interaction between the child and the parent process, you will need to develop a communication protocol to allow them to interact.

A common paradigm uses:

* `stdin` to child as the events passed from the parent.
* `stdout` as requests sent by the child to the parent.
* `stderr` as logging messages, which allows the child to use standard error handling methods to report status to the parent.

The `stdin` and `stdout` communication should work with passing packets.  Things like protobuf or streaming JSON are good candidates to establishing a basis for communication.

The `comm` sub-module offers some basic building blocks to extract packets out of streams.

## License

Grackle Zero is under the [MIT License](LICENSE).
