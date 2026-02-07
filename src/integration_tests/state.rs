//! Communication State Store

use std::sync::{Arc, Mutex};

use crate::runtime::error;

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
}

impl Expected {
    /// The OS or library prevents the executable from even starting.
    pub fn doesnt_start() -> Self {
        Self {
            exit_code: vec![],
            handle_started: false,
            sent_init: false,
            read_start: false,
            read_end: false,
        }
    }

    /// The executable is able to perform all its actions, and returns with a 0 exit code.
    pub fn succeeds() -> Self {
        Self {
            exit_code: vec![0],
            handle_started: true,
            sent_init: true,
            read_start: true,
            read_end: true,
        }
    }

    /// The executable attempts to perform a prohibited behavior but is stopped.
    /// It performs all the protocol behavior, but terminates before it reaches
    /// the sending "completed" status.
    pub fn blocked() -> Self {
        Self {
            exit_code: vec![101, 111], // Seems like the standard Rust exit code for panic.
            handle_started: true,
            sent_init: true,
            read_start: true,
            read_end: false,
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

    pub fn set_exit_code(&self, code: Option<i32>) -> Result<bool, std::io::Error> {
        self.update(|c| match code {
            Some(v) => {
                c.exit_code = Some(v);
                true
            }
            None => false,
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
    pub fn assert(&self, res: Result<i32, error::SandboxError>, expected: Expected) {
        let guard = self.state.lock().expect("lock poisoned");
        guard.ensure(expected, res);
    }
}

#[derive(Debug)]
/// Contains the test communication process.
struct InnerExecutionState {
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

impl InnerExecutionState {
    /// Ensure the expected matches the actual.  Order matters.
    fn ensure(&self, expected: Expected, res: Result<i32, error::SandboxError>) {
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

        match self.exit_code {
            Some(c) => {
                if !expected.exit_code.contains(&c) {
                    if success {
                        // Didn't report the status above.
                        println!("Expected: {:?}", expected);
                        println!("  Actual: {:?}", self);
                    }
                    success = false;
                }
                match res {
                    Ok(r) => {
                        if r != c {
                            println!(
                                "Recorded child exit code {}, runtime returned exit code {}",
                                c, r,
                            );
                            success = false;
                        }
                    }
                    Err(e) => {
                        if expected.exit_code.contains(&0) {
                            println!(
                                "Expected success exit code, but runtime returned error: {:?}",
                                e,
                            );
                            success = false;
                        }
                    },
                    
                }
            }
            None => {
                // This is only bad if the child is expected to fail.
                // If it succeeds, then this means the communication process ended as expected,
                // and the child may have taken some time to complete.
                // FIXME this logic is wrong.  There are times where the child doesn't terminate
                // before the handler notices it.
                let zero = 0i32;
                if !expected.exit_code.contains(&zero) {
                    println!("Error: Child failed to terminate through the execution handler.");
                    success = false;
                } else if res.is_err() {
                    println!(
                        "Expected success exit code, but runtime returned error: {:?}",
                        res.err().unwrap(),
                    );
                    success = false;
                }
            }
        }

        assert!(success, "Execution State mismatch");
    }
}
