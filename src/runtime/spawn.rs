// SPDX-License-Identifier: MIT

//! General model for spawning child processes and managing their state.

use std::{collections::HashMap, ffi::OsString, path::PathBuf};

/// Handles communication to the child from the parent process.
///
/// This is the basic communication method for handling requests from the child process.
pub trait CommHandler {
    fn handle(self, child: Box<dyn Child>) -> Result<(), std::io::Error>;
}

/// Simple method for communicating with the child process.
pub trait Child {
    /// Request a hard termination of the child process.
    fn terminate(&self) -> Result<(), std::io::Error>;

    /// Take the stream that receives from the child, as was marked with the child's FD.
    /// If called again with the same FD, this will return None.
    fn take_stream_from_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Read>>;

    /// Take the stream that sends to the child, as was marked with the child's FD.
    /// If called again with the same FD, this will return None.
    fn take_stream_to_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Write>>;

    /// Get the current exit status for the child process.
    fn exit_status(&self) -> Option<i32>;
}

/// Defines the required file descriptors used in the construction of the child process.
///
/// By default, STDIN is at index 0, STDOUT is at index 1, and STDERR is at index 2.
#[derive(Debug, Clone)]
pub struct FdSet {
    fds: Vec<Fd>,
}

/// The FD mode description, indicating the direction of data.
#[derive(Debug, Clone)]
pub enum FdMode {
    // Used only for the 'basic' format, where the FD is closed in the child but not used for communication.
    Null,
    // The data flows from the parent to the child.
    ToChild,
    // The data flows from the child to the parent.
    FromChild,
    // The FD is kept open in the child without redirection.
    KeepInChild,
}

/// A single file descriptor, which has an index and a direction.
#[derive(Debug, Clone)]
pub struct Fd {
    pub fd: u32,
    pub mode: FdMode,
}

/// File Descriptor set request for the child process.
/// Constructs the consecutive file descriptors passed to the child process.
impl FdSet {
    /// Create a new FdSet using mode definitions, one per slice index.
    /// That is, index 0 is assigned FD 0, index 1 to FD 1, and so on.
    pub fn basic(modes: &[FdMode]) -> Self {
        let mut fds = Vec::with_capacity(modes.len());
        for i in 0..modes.len() {
            fds.push(Fd {
                fd: i as u32,
                mode: modes[i].clone(),
            });
        }
        FdSet { fds }
    }

    /// Construct the file descriptors from the list of values.
    pub fn from_vec(fds: Vec<Fd>) -> Self {
        FdSet { fds }
    }

    /// Construct the file descriptors from an index map.
    pub fn from_map(map: HashMap<u32, FdMode>) -> Self {
        let mut fds = Vec::with_capacity(map.len());
        for e in map.iter() {
            fds.push(Fd {
                fd: *e.0,
                mode: e.1.clone(),
            });
        }
        FdSet { fds }
    }

    /// Define the standard IoRequest, using STDIN, STDOUT, and STDERR.
    pub fn std() -> Self {
        FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::FromChild])
    }

    /// Retrieve the file descriptor modes used in the request.
    pub fn modes(&self) -> Vec<Fd> {
        self.fds.clone()
    }

    pub fn len(&self) -> usize {
        self.fds.len()
    }
}

/// Describes how to launch the child process.
pub struct LaunchEnv {
    pub cmd: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: HashMap<OsString, OsString>,
    pub fds: FdSet,
}
