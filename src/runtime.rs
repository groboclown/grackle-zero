// SPDX-License-Identifier: MIT

//! Manages the runtime execution of the child process, and the communication
//! with the process.
//!
//! The `sandbox_child` function is the main entry point to launch a
//! sandboxed child process.  It takes a `LaunchEnv` structure that describes
//! the command to run, its arguments, environment variables, working directory,
//! and file descriptor mappings.  It takes a `CommHandler` that manages
//! the communication with the child process.  When the `CommHandler` instance
//! exits execution, the child process is terminated if it is still running, and its
//! exit code is returned.

pub mod error;
pub mod spawn;

pub use spawn::{Child, CommHandler, FdMode, FdSet, LaunchEnv};

#[cfg(target_os = "linux")]
mod spawn_linux;

#[cfg(target_os = "linux")]
pub fn sandbox_child<CH: CommHandler>(
    env: LaunchEnv,
    handler: CH,
) -> Result<i32, error::SandboxError> {
    let child = spawn_linux::launch_child(env)?;
    let state = child.state();
    handler.handle(Box::new(child))?;
    state.child_exit_code()
}

#[cfg(target_os = "windows")]
mod spawn_windows;

#[cfg(target_os = "windows")]
pub fn sandbox_child<CH: CommHandler>(
    env: LaunchEnv,
    handler: CH,
) -> Result<i32, error::SandboxError> {
    todo!()
}

#[cfg(target_os = "macos")]
mod spawn_darwin;

#[cfg(target_os = "macos")]
pub fn sandbox_child<CH: CommHandler>(
    env: LaunchEnv,
    handler: CH,
) -> Result<i32, error::SandboxError> {
    todo!()
}

#[cfg(test)]
mod tests {
    // Integration tests that call to the programs in the 'tests' directory.
    // The standard mechanism for the program's execution is:
    //    launch the program with 1 argument.
    //    Write 1 byte to stdout (should be '1').
    //    perform violation.
    //    Write 1 byte to stdout (should be '2').
    // For simplification, it looks for the target/debug/(executable name)
    use super::*;
    use crate::runtime::spawn::{FdMode, FdSet};
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::io::{ErrorKind, Write};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    // Run the noop program.
    #[test]
    fn test_noop() {
        let handler = TestHandler::new();
        let comms = handler.get_comms();

        let res = sandbox_child(
            LaunchEnv {
                cmd: find_exec("noop"),
                args: vec![OsString::from("1")],
                cwd: PathBuf::from("."),
                env: env_backtrace(),
                // This leaves stderr untouched for easier debugging.
                fds: FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild]),
            },
            handler,
        );
        comms.lock().unwrap().ensure(
            TestComms {
                exit_code: Some(0),
                handle_started: true,
                sent_init: true,
                read_start: true,
                read_end: true,
            },
            res,
        );
    }

    // Run the read-file program.
    #[test]
    fn file_read() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "contents").unwrap();

        let handler = TestHandler::new();
        let comms = handler.get_comms();

        let res = sandbox_child(
            LaunchEnv {
                cmd: find_exec("file-read"),
                args: path_as_args(file.path()),
                cwd: PathBuf::from("."),
                env: env_backtrace(),
                // This leaves stderr untouched for error reporting.
                fds: FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild]),
            },
            handler,
        );
        comms.lock().unwrap().ensure(
            TestComms {
                exit_code: Some(101), // seems like rust panic exit code.
                handle_started: true,
                sent_init: true,
                read_start: true,
                read_end: false,
            },
            res,
        );
    }

    // Run the cpuid program.
    #[test]
    fn cpuid() {
        let handler = TestHandler::new();
        let comms = handler.get_comms();

        let res = sandbox_child(
            LaunchEnv {
                cmd: find_exec("cpuid"),
                // This can pass in many different arguments.
                //
                args: vec![OsString::from("soumicdhq")],
                cwd: PathBuf::from("."),
                env: env_backtrace(),
                // This leaves stderr untouched for error reporting.
                fds: FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild]),
            },
            handler,
        );
        comms.lock().unwrap().ensure(
            TestComms {
                exit_code: Some(101), // seems like rust panic exit code.
                handle_started: true,
                sent_init: true,
                read_start: true,
                read_end: false,
            },
            res,
        );
    }

    // Run the exec-self program.
    #[test]
    fn exec_self() {
        let handler = TestHandler::new();
        let comms = handler.get_comms();

        let res = sandbox_child(
            LaunchEnv {
                cmd: find_exec("exec-self"),
                // This can pass in many different arguments.
                //
                args: vec![OsString::from("soumicdhq")],
                cwd: PathBuf::from("."),
                env: env_backtrace(),
                // This leaves stderr untouched for error reporting.
                fds: FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild]),
            },
            handler,
        );
        comms.lock().unwrap().ensure(
            TestComms {
                exit_code: Some(101), // seems like rust panic exit code.
                handle_started: true,
                sent_init: true,
                read_start: true,
                read_end: false,
            },
            res,
        );
    }

    #[derive(Debug)]
    /// Contains the test communication process.
    struct TestComms {
        /// exit_code contains the exit code at the current execution point in the process.
        exit_code: Option<i32>,

        // started is true after the handler has begun running.
        handle_started: bool,

        // sent_init is true after sending the initial byte to stdout.
        sent_init: bool,

        // read_start is true after the first byte from stdout is read.
        read_start: bool,

        // read_end is true after the second byte from stdout is read.
        read_end: bool,
    }

    impl TestComms {
        /// Ensure the expected matches the actual.  Order matters.
        fn ensure(&self, expected: Self, res: Result<i32, error::SandboxError>) {
            let mut success = true;
            if self.exit_code != expected.exit_code
                || self.handle_started != expected.handle_started
                || self.sent_init != expected.sent_init
                || self.read_start != expected.read_start
                || self.read_end != expected.read_end
            {
                println!("Expected: {:?}", expected);
                println!("  Actual: {:?}", self);
                success = false;
            }

            if success && self.exit_code.is_some() && self.exit_code.unwrap() != 0 && res.is_ok() {
                println!(
                    "Expected error exit code {}, but got Ok result",
                    self.exit_code.unwrap()
                );
                success = false;
            }
            if success && self.exit_code.is_some() && self.exit_code.unwrap() == 0 && res.is_err() {
                println!(
                    "Expected ok exit code 0, but got Err result: {}",
                    res.err().unwrap(),
                );
                success = false;
            }

            assert!(success, "TestComms mismatch");
        }
    }

    struct TestHandler {
        comms: Arc<Mutex<TestComms>>,
    }

    impl CommHandler for TestHandler {
        fn handle(self, mut child: Box<dyn spawn::Child>) -> Result<(), std::io::Error> {
            let ret = self.run_process(&mut child);
            match &ret {
                Ok(_) => {
                    println!("Process returned Ok");
                }
                Err(r) => {
                    println!("Process returned: {}", r);
                }
            }
            if !self.update_comms_value(|c| {
                let status = child.exit_status();
                c.exit_code = status;
                status.is_some()
            })? {
                child.terminate()?;
            }
            ret
        }
    }

    impl TestHandler {
        fn new() -> Self {
            TestHandler {
                comms: Arc::new(Mutex::new(TestComms {
                    exit_code: None,
                    handle_started: false,
                    sent_init: false,
                    read_start: false,
                    read_end: false,
                })),
            }
        }

        fn get_comms(&self) -> Arc<Mutex<TestComms>> {
            self.comms.clone()
        }

        /// Generic helper to lock `TestComms` and mutate with a provided closure.
        fn update_comms_value<R, F>(&self, f: F) -> Result<R, std::io::Error>
        where
            F: FnOnce(&mut TestComms) -> R,
        {
            let mut guard = self
                .comms
                .lock()
                .map_err(|_| std::io::Error::new(ErrorKind::BrokenPipe, "lock poisoned"))?;
            Ok(f(&mut *guard))
        }

        fn run_process(&self, child: &mut Box<dyn spawn::Child>) -> Result<(), std::io::Error> {
            self.update_comms_value(|c| {
                c.handle_started = true;
            })?;
            if child.exit_status().is_some() {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    "child exited before communication",
                ));
            }
            let mut out = match child.take_stream_to_child(0) {
                Some(f) => f,
                None => {
                    return Err(std::io::Error::new(ErrorKind::BrokenPipe, "no stdin"));
                }
            };
            let mut inp = match child.take_stream_from_child(1) {
                Some(f) => f,
                None => {
                    return Err(std::io::Error::new(ErrorKind::BrokenPipe, "no stdout"));
                }
            };

            let mut buf = [b'0'];
            out.write_all(&buf)?;
            // Drop the output to signal to the child that the parent is finished writing.
            // This flushes the pipe, and allows the child to read EOF if needed.
            drop(out);
            self.update_comms_value(|c| {
                c.sent_init = true;
            })?;

            inp.read_exact(&mut buf)?;
            if buf[0] != b'1' {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    format!("did not read '1', but {}", buf[0]),
                ));
            }
            self.update_comms_value(|c| {
                c.read_start = true;
            })?;

            inp.read_exact(&mut buf)?;
            if buf[0] != b'2' {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    format!("did not read '2', but {}", buf[0]),
                ));
            }
            self.update_comms_value(|c| {
                c.read_end = true;
            })?;

            Ok(())
        }
    }

    fn path_as_args(path: &Path) -> Vec<OsString> {
        vec![path.into()]
    }

    #[cfg(target_os = "windows")]
    const EXEC_SUFFIX: &str = ".exe";

    #[cfg(not(target_os = "windows"))]
    const EXEC_SUFFIX: &str = "";

    /// Find the executable for the given test program.
    fn find_exec(exec_name: &str) -> PathBuf {
        // Find the 'tests' directory off the root.
        let test_dir = Path::new("tests");
        assert!(test_dir.is_dir());
        let mut exec: PathBuf = test_dir.into();
        exec.push(exec_name);
        let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
        assert!(exec.is_dir(), "did not find test directory ({})?", exec_s);
        exec.push("target");
        let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
        assert!(
            exec.is_dir(),
            "could not find {}; did you remember to run 'cargo build' on it?",
            exec_s,
        );
        exec.push("debug");
        let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
        assert!(
            exec.is_dir(),
            "could not find {}; did you remember to run 'cargo build' on it?",
            exec_s,
        );
        exec.push(format!("{exec_name}{EXEC_SUFFIX}"));
        let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
        assert!(
            exec.is_file(),
            "could not find {}; did you remember to run 'cargo build' on it?",
            exec_s,
        );
        exec
    }

    fn env_backtrace() -> HashMap<OsString, OsString> {
        let mut env = HashMap::new();
        env.insert(OsString::from("RUST_BACKTRACE"), OsString::from("1"));
        env
    }
}
