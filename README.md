# Project Grackle Zero

Execute Tasks in Zero Access Mode.

## Purpose

This library executes other programs in a [zero-access sandbox](#limitations).  It only communicates to the parent process through the file descriptors constructed during setup.  It allows for a program to perform operations dictated by an outside agent that has the possibility of leading to an attack on your system, and give additional defense in depth for the execution.

For example, the id Quake game included a scripting language that was compiled into native code from an embedded C compiler.  While the scripting language allowed for a "safe" subset of C, it still opens the doors for a malicious actor to introduce scripts that can escape the game engine.  Additionally, the script could take advantage of vulnerabilities in the embedded C compiler and perform an escape there.  The Grackle Zero library would allow for running the C compiler and the compiled script within a sandboxed process to add more protections for those components.

The library operates by using the OS provided capabilities to limit the executed program's capabilities.  It does not run them in a virtual machine.  Because this uses usermode techniques, some operating systems allow for some limited access that may not be desired.  As with all security tools, please understand the limitations and advantages of the libraries you choose.  None are a silver bullet.

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

## Limitations

While the library attempts to use many techniques to limit the capabilities of the executed process, different execution environments have limitations to what they can prevent.  Here we describe all known limitations.  If you can identify others, please open an [issue](https://github.com/groboclown/grackle-zero/issues) so we can help the community make better informed decisions when using this library.

### Windows

* Passes environment variables that include the username.  Required due to using AppContainer.
* Permits read access to globally readable registry keys.
* Permits access to the WindowsNT clock APIs.

## Roadmap

### Linux

* [x] Implement execution.
* [x] Clamp down on filesystem read and write access, as well as some limited network restrictions, using Landlock.
* [x] Restrict the kinds of OS syscalls that can be made through SecComp.
* [ ] Add resource restrictions like cgroups and CPU scheduling.
* [ ] Add defense in depth by adding namespace mounts.

### Windows

* [x] Implement execution.
* [x] Lauch the process with a restricted token, limiting the permissions and available SIDs.
* [x] Launch the process inside an AppContainer.  Without this, the application can read and write files on the host and access the network.
* [x] Short-term implementation for handle passing (those that aren't stdin, stdout, stderr) to children.  This passes them in an environment variable in the format `SANDBOX_HANDLES=FD_NUMBER:0xHANDLE_ADDRESS;FD_NUMBER:0xHANDLE_ADDRESS;...`, where the `FD_NUMBER` is the established "file descriptor number" declared in the `FdSet`, and the `HANDLE_ADDRESS` is the handle value in hexadecimal.
* [ ] Disable win32k to prevent UI or kernel-mode window/GUI syscalls.
* [ ] Use alternate desktop / window station to isolate UI.
* [ ] Allow running as another user.  This requires adding the other user and having that user's credentials.  While this can greatly increase security by not revealing the current user's name, and by running with significantly reduced capabilities, it requires administrative access for a one-time user creation.
* [ ] Patch `ntdll.dll`, similar to [Chromium](https://github.com/chromium/chromium/blob/main/sandbox/win/src/interception.cc#L384).
* [ ] Finalize on a method for passing non-standard handles to the child process.

Some known limitations with the Windows imlementation that we have no expectation to change:

* Cannot launch a script file (such as batch).  Instead, this only supports launching a native Windows application.
* Cannot prevent the child program from accessing the system time.

### MacOS

Last on the OS support list.

* [ ] Implement execution.
* [ ] Add a "deny by default" seatbelt profile.

### Examples

The project should include example applications to showcase possibilities of the library.

## License

Grackle Zero is under the [MIT License](LICENSE).
