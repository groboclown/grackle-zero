//! All the executables to test.

use std::io::Write;
use std::path::PathBuf;

use crate::{LaunchEnv, sandbox_child};

use super::handler;
use super::state::Expected;
use super::util;

/// Perform no action.
/// This ensures that, for a program that performs no offending operation,
/// it runs and returns a zero exit code.
#[test]
fn noop() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::find_exec("noop"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
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
            cmd: util::find_exec("file-read"),
            args: util::path_as_args(file.path()),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
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
            cmd: util::find_exec("exec-self"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
        },
        h,
    );
    m.assert(res, Expected::blocked());
}

#[test]
fn cpuid() {
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::find_exec("cpuid"),

            // This can pass in many different arguments.
            args: util::str_as_args("soumicdhq"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
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
    let server = super::server::TcpServer::new().expect("failed to create a TCP/IP server");
    let addr: String = server.addr().to_string();
    let (h, m) = handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::find_exec("tcpip"),
            args: util::string_as_args(&addr),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
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
