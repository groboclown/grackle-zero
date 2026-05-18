// SPDX-License-Identifier: MIT

//! Communication State Store
#[allow(unused)]
use std::sync::{Arc, Mutex};

use gracklezero::runtime::{error, spawn::ExitCode};

#[allow(unused)]
#[derive(Debug)]
pub struct Expected {
    /// The exit code should be one of the listed values.
    pub exit_code: Vec<i32>,

    /// started is true after the handler has begun running.
    pub handle_started: bool,

    /// sent_init is true after sending the initial byte to stdout.
    pub sent_init: bool,

    /// read_start is true after the first byte from stdout is read.
    pub read_start: bool,

    /// read_end is true after the second byte from stdout is read.
    pub read_end: bool,

    /// The sandbox returned an error instead of an exit code.
    pub sandbox_error: bool,
}

impl Expected {
    /// The executable is able to perform all its actions, and returns with a 0 exit code.
    #[allow(unused)]
    pub fn succeeds() -> Self {
        Self {
            exit_code: vec![0],
            handle_started: true,
            sent_init: true,
            read_start: true,
            read_end: true,
            sandbox_error: false,
        }
    }

    /// The executable attempts to perform a prohibited behavior but is stopped.
    /// It performs all the protocol behavior, but terminates before it reaches
    /// the sending "completed" status.
    #[allow(unused)]
    pub fn blocked() -> Self {
        Self {
            exit_code: vec![101, 111], // Seems like the standard Rust exit code for panic.
            handle_started: true,
            sent_init: true,
            read_start: true,
            read_end: false,
            sandbox_error: false,
        }
    }
}

pub struct ExecutionState {
    state: Arc<Mutex<InnerExecutionState>>,
}

impl ExecutionState {
    pub fn new() -> Self {
        ExecutionState {
            state: Arc::new(Mutex::new(InnerExecutionState {
                exit_code: None,
                handle_started: false,
                sent_init: false,
                read_start: false,
                read_end: false,
            })),
        }
    }

    pub fn monitor(&self) -> HandlerCheck {
        HandlerCheck {
            state: self.state.clone(),
        }
    }

    /// Mark that the handle function started running.
    pub fn mark_handle_started(&self) -> Result<(), std::io::Error> {
        self.update(|c| {
            c.handle_started = true;
        })
    }

    /// Mark that the initial data was sent to the child.
    pub fn mark_initial_send(&self) -> Result<(), std::io::Error> {
        self.update(|c| {
            c.sent_init = true;
        })
    }

    /// Mark that the child's signal that it is about to start execution was received.
    pub fn mark_child_started(&self) -> Result<(), std::io::Error> {
        self.update(|c| {
            c.read_start = true;
        })
    }

    /// Mark that the child's signal that it is completed execution was received.
    pub fn mark_child_ended(&self) -> Result<(), std::io::Error> {
        self.update(|c| {
            c.read_end = true;
        })
    }

    pub fn set_exit_code(&self, code: ExitCode) -> Result<bool, std::io::Error> {
        println!("Setting exit code: {:?}", code);
        self.update(|c| {
            let ret = match code {
                ExitCode::Running => false,
                _ => true,
            };
            c.exit_code = Some(code);
            ret
        })
    }

    /// Generic helper to lock the inner state and mutate with a provided closure.
    fn update<R, F>(&self, f: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(&mut InnerExecutionState) -> R,
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        Ok(f(&mut *guard))
    }
}

/// Allows examination of the state of the handler after it completes.
pub struct HandlerCheck {
    state: Arc<Mutex<InnerExecutionState>>,
}

impl HandlerCheck {
    /// Assert that the handler's actual state meets the test's expectations.
    #[allow(unused)]
    pub fn assert(&self, res: Result<ExitCode, error::SandboxError>, expected: Expected) {
        let guard = self.state.lock().expect("lock poisoned");
        guard.ensure(expected, res);
    }

    /// Return true if the handler's actual state meets the test's expectations.
    #[allow(unused)]
    pub fn is_success(&self, res: Result<ExitCode, error::SandboxError>, expected: Expected) -> bool {
        let guard = self.state.lock().expect("lock poisoned");
        guard.is_success(expected, res)
    }
}

#[derive(Debug)]
/// Contains the test communication process.
struct InnerExecutionState {
    /// exit_code contains the exit code at the current execution point in the process.
    exit_code: Option<ExitCode>,

    // started is true after the handler has begun running.
    handle_started: bool,

    // sent_init is true after sending the initial byte to stdout.
    sent_init: bool,

    // read_start is true after the first byte from stdout is read.
    read_start: bool,

    // read_end is true after the second byte from stdout is read.
    read_end: bool,
}

impl InnerExecutionState {
    /// Check the result for issues
    #[allow(unused)]
    fn is_success(&self, expected: Expected, res: Result<ExitCode, error::SandboxError>) -> bool {
        let mut success = true;
        if self.handle_started != expected.handle_started
            || self.sent_init != expected.sent_init
            || self.read_start != expected.read_start
            || self.read_end != expected.read_end
        {
            println!("Expected: {:?}", expected);
            println!("  Actual: {:?}", self);
            success = false;
        }

        if let Err(e) = res {
            println!("Sandbox returned an error: {}", e);
            if !expected.sandbox_error {
                println!("Expected: {:?}", expected);
                println!("  Actual: SandboxError");
                success = false;
            }
        }

        match &self.exit_code {
            None => {
                // The "check for exit code" was never called.
                // The self.state vs. expected state handles the success checks.
            }
            Some(c) => match c {
                ExitCode::Running => {
                    // The child process hasn't exited yet.
                    // This means a bug with the test or the runtime.
                    success = false;
                    println!("The child did not stop (and is most likely still running)");
                }
                ExitCode::OsError(s) => {
                    // Due to OS differences, this can be the equivalent of "never started".
                    // ... but, we'll count the "code" as one of the expected exit codes.
                    let icode = s.code as i32;
                    if !expected.exit_code.contains(&icode) {
                        if success {
                            // Didn't report the status above.
                            println!("Expected: {:?}", expected);
                            println!("  Actual: {:?} ({:?})", self, s);
                        }
                        success = false;
                    }
                }
                ExitCode::Exited(c) => {
                    if !expected.exit_code.contains(&c) {
                        if success {
                            // Didn't report the status above.
                            println!("Expected: {:?}", expected);
                            println!("  Actual: {:?}", self);
                        }
                        success = false;
                    }
                }
            },
        }

        success
    }

    /// Ensure the expected matches the actual.  Order matters.
    #[allow(unused)]
    fn ensure(&self, expected: Expected, res: Result<ExitCode, error::SandboxError>) {
        assert!(self.is_success(expected, res), "Execution State mismatch");
    }
}
