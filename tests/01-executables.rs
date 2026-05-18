// SPDX-License-Identifier: MIT

//! All the test executables that include interaction.

use std::io::Write;
use std::path::PathBuf;

use gracklezero::{LaunchEnv, compat_restrictions, sandbox_child};

mod common;
use common::{gen_r::APP_NAME, handler, server::TcpServer, state::Expected, util};

/// Perform no action.
/// This ensures that, for a program that performs no offending operation,
/// it runs and returns a zero exit code.
#[test]
fn noop() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("noop"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    m.assert(res, Expected::succeeds());
}

/// Read from a file.
/// The test creates a temporary file, then asks the executable
/// to read it.  The executable should be prohibited from reading
/// any file other than itself an its dependent shared libraries.
#[test]
fn file_read() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "contents").unwrap();

    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("file-read"),
            args: util::path_as_args(file.path()),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    m.assert(res, Expected::blocked());
}

/// Execute itself.
/// Because the executable can read its own file (necessary in order to
/// have the 'fork' follow by an 'exec'), have the executable turn around and
/// spawn itself as another executable.
#[test]
fn exec_self() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("exec-self"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    m.assert(res, Expected::blocked());
}

/// Read from the OS clipboard.
#[test]
fn clipboard() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("clipboard"),

            // Argument is the number of attempts to read.
            // Give it 3 tries in case it encounters an "in use" transient error.
            args: util::str_as_args("3"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    m.assert(res, Expected::blocked());
}

/// Run system queries to ensure the sandbox blocks all of them.
/// This test will remain disabled until all OSes block these calls;
/// to track this, they exist in the top-level README
//#[test]
fn cpuid() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("cpuid"),

            // This can pass in many different arguments.
            // Explicitly leaving out 'o', because that's easy to find out
            // without any special privileges.
            args: util::str_as_args("sumicdhq"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    m.assert(res, Expected::blocked());
}

/// Connect to a TCP/IP server.
/// Have the test create a TCP/IP server on the localhost, then ask the
/// executable to connect to it.
#[test]
fn tcpip() {
    let server = TcpServer::new().expect("failed to create a TCP/IP server");
    let addr: String = server.addr().to_string();
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("tcpip"),
            args: util::string_as_args(&addr),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    let connection_count = server.shutdown().expect("TCP/IP server shutdown failed");
    assert_eq!(
        connection_count, 0,
        "The child could connect to the local TCP/IP server at {}",
        &addr,
    );
    m.assert(res, Expected::blocked());
}

/// Run a GUI application.
#[test]
fn gui() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("gui"),
            args: util::str_as_args("app"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!(APP_NAME),
        },
        h,
    );
    // Note that on Windows this may run just fine, because the GUI API
    // calls aren't blocked; they just prevent the GUI from showing.
    // To have GUI calls turn into failures requires adding in the DLL shim to
    // trampoline the call to a failure.
    #[cfg(target_os = "windows")]
    m.assert(res, Expected::succeeds());

    #[cfg(not(target_os = "windows"))]
    m.assert(res, Expected::blocked());
}
