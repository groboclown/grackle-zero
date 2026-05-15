# Project Grackle Zero

Execute Tasks in Zero Access Mode, or as close as the OS allows.

## Purpose

This library executes other programs in a [zero-access sandbox](#limitations).  It only communicates to the parent process through the file descriptors constructed during setup.  It allows for a program to perform operations dictated by an outside agent that has the possibility of leading to an attack on your system, and give additional defense in depth for the execution.

**WARNING** The library operates by using the OS provided capabilities to limit the executed program's capabilities.  It does not run them in a virtual machine.  Because this uses usermode techniques, some operating systems allow for some limited access that may not be desired.  As with all security tools, please understand the limitations and advantages of the libraries you choose.  None are a silver bullet.

### Deeper Dive

Just wrapping the execution in an in-process virtual language (such as a JavaScript or Lua interpreter) does not guarantee that the code cannot perform malicious actions.  As the [Google Chrome security team discuss]([https://arxiv.org/pdf/1902.05178]), with the discovery of Sectre and Meltdown attacks,

> speculative vulnerabilities on today’s hardware defeat all language-enforced confidentiality with no known comprehensive software mitigations, as we have discovered that untrusted code can construct a universal read gadget to read all memory in the same address space through side-channels.  In the face of this reality, we have shifted the security model of the Chrome web browser and V8 to process isolation.

Therefore, this project aims to allow programs to execute potentially hazardous operations (such as the execution of user-provided scripts) inside a sandbox to further limit these side-channel attacks.

For example, the id game "Quake" included a scripting language that the game compiled into native code from an embedded C compiler.  While the scripting language allowed for a "safe" subset of C, it still opens the doors for a malicious actor to introduce scripts that can escape the game engine.  Additionally, the script could take advantage of vulnerabilities in the embedded C compiler and perform an escape there.  The Grackle Zero library would allow for running the C compiler and the compiled script within a sandboxed process to add more protections for those components.

### High Levels of Configuration Equivalent to Scripting

The more flexibility we grant end users to change the logic flow in our applications, the more power we give them to exercise unexpected routes in the code, thus opening pathways for exposing bugs and attack vectors.  Scripting languages just happen to shine as the most obvious example.

However, the security community has seen that even granting XOR capabilities to image generation can lead to touring complete [strange machines](https://projectzero.google/2021/12/a-deep-dive-into-nso-zero-click.html).

## Use

The library provides a basic framework for building a communication pattern, which you can build upon to monitor and interact with the child process.

Here's a trivial example of launching a program within a sandbox, then interacting with it by sending data to its `stdin`, and reading from its `stdout`.  It intentionally leaves its `stderr` untouched, so that any message sent to the child program's `stderr` is also output through the parent program's `stderr`.

```rust
use gracklezero::{sandbox_child, CommHandler, LaunchEnv, compat_restrictions};
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
          restrictions: compat_restrictions!("sandbox"),
      },
      handler,
  ).expect("the sandbox execution should not cause an error");
  println!("Child exited with {}", exit_code);
}
```

### Additional Restriction Control

The application you try to launch as a sandboxed child may have additional OS requirements necessary to allow it to run.  To give you more control in managing these, you can use the [`restrictions`](src/restrictions.rs).

Generally, you will should look at using the built-in macros, either `compat_restrictions!` or `strict_restrictions!` to get started.  The `strict_restrictions` uses a standard set of restriction that grants fairly secure limits on the process, but not every possible one (some Windows capabilities can prevent nearly every executable from running).  The `compat_restrictions` attempts to enforce the same restrictions, while also guaranteeing that version changes in the grackle-zero library does not make the restrictions stronger, and, thus, allowing executables that used to run to continue to run after upgrade.

To adjust the standard restrictions, you pass in either a provided helper to toggle a setting, or a function with arguments, or pass in an explicit function call:

```rust
  let r: gracklezero::Restrictions = gracklezero::strict_restrictions!(
    "my-sandbox-wrapper",

    // Toggle the kill process setting.
    gracklezero::restrictions::linux::kill_process_on_seccomp_violation,

    // Set the maximum files, by passing in a tuple the function to call and
    // its arguments.
    (
        linux::with_max_open_files,
        4096,
    ),

    // Set the maximum files using an explicit function call.
    // The 'r' argument is the Restrictions object.
    |r| { linux::with_max_open_files(r, 4096) },
  );
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

### Windows Limitations

* Passes environment variables that include the username.  Required due to using AppContainer.
* Permits read access to globally readable registry keys.
* Permits access to the Windows `kernel32.dll`, `gdi.dll` and `user32.dll` (Win32k) libraries.  While the sandbox locks some capabilities within these down, it currently does not prevent some calls that it should.  See the [roadmap below](#windows-roadmap) for more details on the necessary effort to enable this.


## Roadmap

### Linux Roadmap

* [x] Implement execution.
* [x] Clamp down on filesystem read and write access, as well as some limited network restrictions, using Landlock.
* [x] Restrict the kinds of OS syscalls that can be made through SecComp.
* [ ] Add resource restrictions like cgroups and CPU scheduling.
* [ ] Add defense in depth by adding namespace mounts.

### Windows Roadmap

* [x] Implement execution.
* [x] Lauch the process with a restricted token, limiting the permissions and available SIDs.
* [x] Launch the process inside an AppContainer.  Without this, the application can read and write files on the host and access the network.
* [x] Short-term implementation for handle passing (those that aren't stdin, stdout, stderr) to children.  This passes them in an environment variable in the format `SANDBOX_HANDLES=FD_NUMBER:0xHANDLE_ADDRESS;FD_NUMBER:0xHANDLE_ADDRESS;...`, where the `FD_NUMBER` is the established "file descriptor number" declared in the `FdSet`, and the `HANDLE_ADDRESS` is the handle value in hexadecimal.
* [x] Use alternate desktop / window station to isolate UI.
* [ ] Allow running as another user.  This requires adding the other user and having that user's credentials.  While this can greatly increase security by not revealing the current user's name, and by running with significantly reduced capabilities, it requires administrative access for a one-time user creation.
* Calls into DLLs should use a "shim", similar to how similar to [Chromium](https://github.com/chromium/chromium/blob/main/sandbox/win/src/interception.cc#L384) does this.  This technique allows fine control over allowed or disallowed API calls.  The Chrome team [went into detail](https://projectzero.google/2016/11/breaking-chain.html) about the issues with native Windows APIs, and how their team went about allowing certain calls.
  * [ ] Disable registry queries.
  * [ ] Disable inspection of system ID.
  * [ ] Disable kernel queries for the current user name and system information (for example, how the [whoami](https://github.com/ardaku/whoami/blob/v2/whoami/src/os/windows.rs) crate inspects the system).
* [x] Finalize on a method for passing non-standard handles to the child process.

Some known limitations with the Windows imlementation that we have no expectation to change:

* Cannot launch a script file (such as batch).  Instead, this only supports launching a native Windows application.
* Graphical applications launch and assume that the user can interact with the UI elements.  When run with the desktop isolation enabled, the user interface does not appear to the user.

### MacOS

Last on the OS support list.

* [ ] Implement execution.
* [ ] Add a "deny by default" seatbelt profile.

### Examples

The project should include more example applications to showcase possibilities of the library.

## License

Grackle Zero is under the [MIT License](LICENSE).
