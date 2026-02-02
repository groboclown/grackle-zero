//! CommHandler implementation for the tests.

use std::io::ErrorKind;

use super::state::{ExecutionState, HandlerCheck};
use crate::{Child, CommHandler};

/// Create the CommHandler and HandlerCheck for the test execution.
pub fn new() -> (TestHandler, HandlerCheck) {
    let state = ExecutionState::new();
    let check = state.monitor();
    (TestHandler { state }, check)
}

pub struct TestHandler {
    state: ExecutionState,
}

impl CommHandler for TestHandler {
    fn handle(self, mut child: Box<dyn Child>) -> Result<(), std::io::Error> {
        let ret = self.run_process(&mut child);
        match &ret {
            Ok(_) => {
                println!("Process returned Ok");
            }
            Err(r) => {
                println!("Process returned: {}", r);
            }
        }

        // There might be a timing issue here - where the child sent the exit
        // message but still hasn't finished.
        if !self.state.set_exit_code(child.exit_status())? {
            child.terminate()?;
        }
        ret
    }
}

impl TestHandler {
    /// Run the communication process with the child.
    fn run_process(&self, child: &mut Box<dyn Child>) -> Result<(), std::io::Error> {
        self.state.mark_handle_started()?;
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
        self.state.mark_initial_send()?;

        inp.read_exact(&mut buf)?;
        if buf[0] != b'1' {
            return Err(std::io::Error::new(
                ErrorKind::BrokenPipe,
                format!("did not read '1', but {}", buf[0]),
            ));
        }
        self.state.mark_child_started()?;

        inp.read_exact(&mut buf)?;
        if buf[0] != b'2' {
            return Err(std::io::Error::new(
                ErrorKind::BrokenPipe,
                format!("did not read '2', but {}", buf[0]),
            ));
        }
        self.state.mark_child_ended()?;

        Ok(())
    }
}
