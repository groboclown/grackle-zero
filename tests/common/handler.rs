// SPDX-License-Identifier: MIT

//! CommHandler implementation for the tests.
use std::{io::ErrorKind, thread, time::Duration};

use super::state::{ExecutionState, HandlerCheck};
use gracklezero::{Child, CommHandler, runtime::spawn::ExitCode};

/// Create the CommHandler and HandlerCheck for the test execution.
#[allow(unused)]
pub fn new() -> (TestHandler, HandlerCheck) {
    let state = ExecutionState::new();
    let check = state.monitor();
    (TestHandler { state }, check)
}

#[allow(unused)]
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
                println!("Process returned: {:?}", r);
            }
        }

        // There are sometimes timing issues here - where the child sent the exit
        // message but still hasn't finished.
        if !self.state.set_exit_code(child.exit_status())? {
            // The child may have completed protocol I/O but not fully exited yet.
            // Give it a short grace period before forcefully terminating.
            for _ in 0..50 {
                thread::sleep(Duration::from_millis(10));
                if self.state.set_exit_code(child.exit_status())? {
                    return ret;
                }
            }
            println!("Child is still running, terminating");
            match child.terminate() {
                Ok(_) => {
                    println!("Termination successful");
                }
                Err(e) => {
                    println!("Termination failed: {}", e);
                }
            }
            if !self.state.set_exit_code(child.exit_status())? {
                println!("Child is still running after termination");
            } else {
                println!("Child exited after termination");
            }
        }
        ret
    }
}

impl TestHandler {
    /// Run the communication process with the child.
    fn run_process(&self, child: &mut Box<dyn Child>) -> Result<(), std::io::Error> {
        println!("Starting communication with child");
        self.state.mark_handle_started()?;
        println!("Initial exit status check");
        match child.exit_status() {
            ExitCode::OsError(s) => {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    format!(
                        "child exited before communication: {} (0x{:X})",
                        s.message, s.code
                    ),
                ));
            }
            ExitCode::Exited(code) => {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    format!("child exited before communication with code {}", code),
                ));
            }
            ExitCode::Running => {}
        }
        println!("Getting to-child stream");
        let mut out = match child.take_stream_to_child(0) {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(ErrorKind::BrokenPipe, "no stdin"));
            }
        };
        println!("Getting from-child stream");
        let mut inp = match child.take_stream_from_child(1) {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(ErrorKind::BrokenPipe, "no stdout"));
            }
        };

        println!("Writing 1 byte to child");
        write_byte(&mut out, b'1')?;
        // Drop the output to signal to the child that the parent is finished writing.
        // This flushes the pipe, and allows the child to read EOF if needed.
        println!("Closing stdout to child");
        drop(out);
        println!("Marking initial send");
        self.state.mark_initial_send()?;

        println!("Reading start byte");
        match read_byte(&mut inp)? {
            Some(b) => {
                if b != b'1' {
                    return Err(std::io::Error::new(
                        ErrorKind::BrokenPipe,
                        format!("did not read '1', but {}", b),
                    ));
                }
            }
            None => {
                // Closed pipe means the child exited (or closed the fd) before sending the start byte.
                // Therefore, allow the child exit code to take over rather than report an error.
                println!("Child closed the pipe before sending the start byte");
                return Ok(());
            }
        }
        println!("Marking child started");
        self.state.mark_child_started()?;

        println!("Reading end byte");
        match read_byte(&mut inp)? {
            Some(b) => {
                if b != b'2' {
                    return Err(std::io::Error::new(
                        ErrorKind::BrokenPipe,
                        format!("did not read '2', but {}", b),
                    ));
                }
            }
            None => {
                // Closed pipe means the child exited (or closed the fd) before sending the end byte.
                // Therefore, allow the child exit code to take over rather than report an error.
                println!("Child closed the pipe before sending the end byte");
                return Ok(());
            }
        }
        println!("Marking child ended");
        self.state.mark_child_ended()?;

        Ok(())
    }
}

fn read_byte(inp: &mut dyn std::io::Read) -> Result<Option<u8>, std::io::Error> {
    let mut buf = [0];
    match inp.read_exact(&mut buf) {
        Ok(()) => Ok(Some(buf[0])),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(None),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

fn write_byte(out: &mut dyn std::io::Write, byte: u8) -> Result<bool, std::io::Error> {
    let buf = [byte];
    match out.write_all(&buf) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(false),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}
