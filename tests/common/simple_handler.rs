// SPDX-License-Identifier: MIT

//! CommHandler implementation for the tests.

#[allow(unused)]
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gracklezero::{Child, CommHandler, runtime::spawn::ExitCode};

/// Create the CommHandler and HandlerCheck for the test execution.
#[allow(unused)]
pub fn new() -> (TestHandler, TestMonitor) {
    let state = TestState {
        state: Arc::new(Mutex::new(InnerTestState {
            started: false,
            exit_code: ExitCode::Running,
        })),
    };
    (
        TestHandler {
            timeout: Duration::from_secs(10),
            sleep: Duration::from_millis(200),
            state: state.clone(),
        },
        TestMonitor { state },
    )
}

#[allow(unused)]
pub struct TestMonitor {
    state: TestState,
}

impl TestMonitor {
    #[allow(unused)]
    pub fn did_start(&self) -> bool {
        self.state.started()
    }

    #[allow(unused)]
    pub fn assert_never_started(&self) {
        assert_eq!(self.state.started(), false);
    }

    #[allow(unused)]
    pub fn exit_code(&self) -> ExitCode {
        self.state.exit_code().clone()
    }

    #[allow(unused)]
    pub fn assert_failed(&self, err: &str) {
        assert_eq!(self.state.started(), true);
        match &self.state.exit_code() {
            ExitCode::OsError(term) => {
                assert_eq!(format!("{:?}", term), err);
            }
            ExitCode::Exited(c) => {
                panic!("exited with code {}", c)
            }
            ExitCode::Running => {
                panic!("still running after timeout");
            }
        }
    }

    #[allow(unused)]
    pub fn assert_exited_with(&self, code: i32) {
        assert_eq!(self.state.started(), true);
        match &self.state.exit_code() {
            ExitCode::Exited(c) => {
                assert_eq!(*c, code);
            }
            ExitCode::OsError(term) => {
                panic!("terminated due to {:?}", term)
            }
            ExitCode::Running => {
                panic!("still running after timeout");
            }
        }
    }
}

pub struct TestHandler {
    sleep: Duration,
    timeout: Duration,
    state: TestState,
}

impl CommHandler for TestHandler {
    fn handle(self, child: Box<dyn Child>) -> Result<(), std::io::Error> {
        self.state.set_started();

        // Wait until timeout or exit.
        let expires = Instant::now() + self.timeout;
        loop {
            let exit = child.exit_status();
            match exit {
                ExitCode::Running => (),
                x => {
                    self.state.set_exit_code(x);
                    break;
                }
            }
            if Instant::now() >= expires {
                break;
            }
            std::thread::sleep(self.sleep);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct InnerTestState {
    started: bool,
    exit_code: ExitCode,
}

#[derive(Clone)]
struct TestState {
    state: Arc<Mutex<InnerTestState>>,
}

impl TestState {
    pub fn started(&self) -> bool {
        self.update(|s| s.started)
    }

    pub fn exit_code(&self) -> ExitCode {
        self.update(|s| s.exit_code.clone())
    }

    pub fn set_started(&self) {
        self.update(|s| {
            s.started = true;
        });
    }

    pub fn set_exit_code(&self, code: ExitCode) {
        self.update(|s| {
            s.exit_code = code;
        });
    }

    /// Generic helper to lock the inner state and mutate with a provided closure.
    fn update<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut InnerTestState) -> R,
    {
        let mut guard = self.state.lock().expect("lock poisoned");
        f(&mut *guard)
    }
}
