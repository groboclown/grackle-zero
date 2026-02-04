// SPDX-License-Identifier: MIT

//! Construct file descriptors for the passing between the parent and child processes.

use std::{
    collections::HashSet, fs::File, os::fd::{AsRawFd, OwnedFd, RawFd}
};

use nix::{libc::dup2, unistd::pipe};

use crate::runtime::{error::SandboxError, spawn::FdSet};

pub struct ForkedFd {
    fds: Vec<FdForkMap>,
    keep_fds: HashSet<nix::libc::c_int>,
}

#[derive(Debug, Clone, Copy)]
pub enum StreamDirection {
    ToChild,
    FromChild,
}

/// Maps the FD as requested that the child sees the connection + the stream to
/// talk with the child.
pub struct FdMap {
    pub dup_to: u32,
    pub stream: File,
    pub direction: StreamDirection,
}

impl ForkedFd {
    /// Construct the new forked FD mappings.
    /// This will construct the FIFO pipes as needed.
    pub fn new(config: FdSet) -> Result<Self, SandboxError> {
        let mut fds: Vec<FdForkMap> = Vec::new();
        let mut keep_fds: HashSet<nix::libc::c_int> = HashSet::new();

        for fd_m in config.modes() {
            match fd_m.mode {
                crate::runtime::spawn::FdMode::Null => {},
                crate::runtime::spawn::FdMode::KeepInChild => {
                    // Keep the FD open in the child without redirection.
                    keep_fds.insert(fd_m.fd as nix::libc::c_int);
                }
                crate::runtime::spawn::FdMode::FromChild => {
                    let (read_fd, write_fd) = pipe().map_err(|e| errno_to_error(e))?;
                    fds.push(FdForkMap {
                        dup_to: fd_m.fd,
                        parent_fd: read_fd,
                        child_fd: write_fd,
                        direction: StreamDirection::FromChild,
                    });
                    keep_fds.insert(fd_m.fd as nix::libc::c_int);
                }
                crate::runtime::spawn::FdMode::ToChild => {
                    let (read_fd, write_fd) = pipe().map_err(|e| errno_to_error(e))?;
                    fds.push(FdForkMap {
                        dup_to: fd_m.fd,
                        parent_fd: write_fd,
                        child_fd: read_fd,
                        direction: StreamDirection::ToChild,
                    });
                    keep_fds.insert(fd_m.fd as nix::libc::c_int);
                }
            }
        }
        Ok(ForkedFd { fds, keep_fds })
    }

    /// Get the list of FDs that the child process will use.
    pub fn child_fd_list(&self) -> HashSet<nix::libc::c_int> {
        self.keep_fds.clone()
    }

    /// Called by the parent process after fork, to retrieve the parent process's version of the FDs.
    /// This will drop the child's end of the pipes.
    pub fn parent_after_fork(self) -> Vec<FdMap> {
        let mut ret = Vec::new();
        for fd in self.fds {
            ret.push(fd.parent_after_fork());
        }
        ret
    }

    /// Called by the child process after fork, to prepare the file descriptors.
    /// Because this must run after the fork, which means after the FD no
    /// longer connect to any form of direct logging, errors cause an immediate
    /// exit.  It must also be careful to not allocate memory.
    pub fn child_after_fork(self) {
        // Loop through all the FDs to ensure proper closing of FDs, even on error.
        for fd in self.fds {
            fd.child_after_fork();
        }
    }
}

struct FdForkMap {
    dup_to: u32,
    /// FD used by the parent.
    parent_fd: OwnedFd,
    /// FD used by the child.
    child_fd: OwnedFd,
    direction: StreamDirection,
}

impl FdForkMap {
    /// Handle the FD mapping for the child process.
    /// Duplicate the FD to the dup_to, and close both fd and also_close.
    /// Because this must run after the fork, which means after the FD no
    /// longer connect to any form of direct logging, errors cause an immediate
    /// exit.   It must also be careful to not allocate memory.
    fn child_after_fork(self) {
        // Because this passes ownership (self, not &self), + this uses OwnedFd,
        // returning from this function will cause OwnedFd to drop, and thus be closed.
        // The self.child_fd.as_raw_fd() uses a &self, so ownership does not get lost
        // until this exits.
        let dup_to = self.dup_to;

        // dup2 is an unsafe method, as is getting the raw FD.
        let res = unsafe { dup2(self.child_fd.as_raw_fd(), dup_to as RawFd) };
        // dup2 returns the new fd (dup_to) on success, and -1 on error.
        if res < 0 {
            std::process::exit(253);
        }
    }

    // Handle the FD mapping for the parent process.
    // Closes the also_close FD, and returns the fd as a stream,
    // which passes ownership of the OwnedFd to the stream object, which prevents
    // it from closing.
    fn parent_after_fork(self) -> FdMap {
        FdMap {
            dup_to: self.dup_to,
            stream: File::from(self.parent_fd),
            direction: self.direction,
        }
    }
}

fn errno_to_error(err: nix::Error) -> SandboxError {
    SandboxError::Io(err.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::spawn::{Fd, FdMode, FdSet};
    use nix::libc;
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd::{fork, ForkResult, Pid};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;

    /// Test that the parent mapping direction construction are correct.
    #[test]
    fn parent_mapping_directions() {
        // Build an FdSet with mixed modes including Null.
        // Make sure the FDs aren't consecutive.
        let fds = FdSet::from_vec(vec![
            Fd { fd: 5, mode: FdMode::ToChild },
            Fd { fd: 7, mode: FdMode::Null },
            Fd { fd: 12, mode: FdMode::FromChild },
        ]);

        // Create forked fds, then simulate the parent path to collect streams.
        let forked = ForkedFd::new(fds).expect("Failed to create ForkedFd");
        let mut maps = forked.parent_after_fork();

        // Only non-Null entries should result in mappings.
        assert_eq!(maps.len(), 2, "expected exactly 2 active mappings");

        // Sort by dup target for stable assertions.
        maps.sort_by_key(|m| m.dup_to);

        assert_eq!(maps[0].dup_to, 5);
        matches_direction(&maps[0], StreamDirection::ToChild);

        assert_eq!(maps[1].dup_to, 12);
        matches_direction(&maps[1], StreamDirection::FromChild);

    }

    /// Test data flowing through stdin to the child process.
    #[test]
    fn to_child_data_flow_via_stdin() {
        let fds = FdSet::from_vec(vec![Fd { fd: 0, mode: FdMode::ToChild }]);
        let forked = ForkedFd::new(fds).expect("Failed to create ForkedFd");

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: obtain the write end to send data to child.
                let maps = forked.parent_after_fork();
                assert_eq!(maps.len(), 1);
                let writer = maps.into_iter().next().expect("missing one element");
                matches_direction(&writer, StreamDirection::ToChild);
                let mut writer = writer.stream;
                writer.write_all(b"OK").expect("parent write failed");
                // Explicitly drop to close writer and let child see EOF if needed.
                drop(writer);

                // Reap child and assert it exited successfully.
                assert_child_exit_ok(child);
            }
            Ok(ForkResult::Child) => {
                // Child: install dup2 mappings, then read from FD 0.
                forked.child_after_fork();
                let mut buf = [0u8; 2];
                let mut f = unsafe { File::from_raw_fd(0) };
                exit_on_err(f.read_exact(&mut buf));
                if buf != *b"OK" {
                    exit_with(2);
                }
                exit_ok();
            }
            Err(e) => panic!("fork failed: {}", e),
        }
    }

    /// Test data flowing through stdout from the child process.
    #[test]
    fn from_child_data_flow_via_stdout() {
        let fds = FdSet::from_vec(vec![Fd { fd: 1, mode: FdMode::FromChild }]);
        let forked = ForkedFd::new(fds).expect("Failed to create ForkedFd");

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: obtain the write end to send data to child.
                let maps = forked.parent_after_fork();
                assert_eq!(maps.len(), 1);
                let reader = maps.into_iter().next().expect("missing one element");
                matches_direction(&reader, StreamDirection::FromChild);
                assert_eq!(reader.dup_to, 1);
                let mut reader = reader.stream;
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).expect("parent read failed");
                assert_eq!(buf, b"OK", "unexpected data from child");

                // Reap child and assert it exited successfully.
                assert_child_exit_ok(child);
            }
            Ok(ForkResult::Child) => {
                // Child: install dup2 mappings, then write to FD 1.
                forked.child_after_fork();
                let buf = *b"OK";
                let mut f = unsafe { File::from_raw_fd(1) };
                exit_on_err(f.write_all(&buf));
                exit_on_err(f.flush());
                exit_ok();
            }
            Err(e) => panic!("fork failed: {}", e),
        }
    }

    /// Test data flowing in both directions using non-standard FDs.
    #[test]
    fn from_child_data_flow_in_out() {
        let fds = FdSet::from_vec(vec![
            Fd { fd: 17, mode: FdMode::FromChild },
            Fd { fd: 21, mode: FdMode::ToChild },
        ]);
        let forked = ForkedFd::new(fds).expect("Failed to create ForkedFd");

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: obtain both ends to communicate with child.
                let maps = forked.parent_after_fork();
                assert_eq!(maps.len(), 2);
                let mut maps = maps.into_iter();
                let reader = maps.next().expect("missing first element");
                matches_direction(&reader, StreamDirection::FromChild);
                assert_eq!(reader.dup_to, 17);
                let mut reader = reader.stream;

                let writer = maps.next().expect("missing second element");
                matches_direction(&writer, StreamDirection::ToChild);
                assert_eq!(writer.dup_to, 21);
                let mut writer = writer.stream;

                // Read from fd 17 (as the child sees the FD).
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).expect("parent read failed");
                assert_eq!(buf, b"AK", "unexpected data from child");

                // Write to fd 21 (as the child sees the FD).
                writer.write_all(b"OK").expect("parent write failed");
                // Explicitly drop to close writer and let child see EOF if needed.
                drop(writer);

                // Reap child and assert it exited successfully.
                assert_child_exit_ok(child);
            }
            Ok(ForkResult::Child) => {
                // Child: install dup2 mappings.
                forked.child_after_fork();

                // Write to fd 17.
                let mut buf = *b"AK";
                let mut f = unsafe { File::from_raw_fd(17) };
                exit_on_err(f.write_all(&buf));
                exit_on_err(f.flush());
                drop(f);  // because of the "read_to_end", need to close the FD.

                // Read from fd 21.
                let mut f = unsafe { File::from_raw_fd(21) };
                exit_on_err(f.read_exact(&mut buf));
                if buf != *b"OK" {
                    exit_with(2);
                }

                exit_ok();
            }
            Err(e) => panic!("fork failed: {}", e),
        }
    }

    // Match the map's direction.
    // Avoids pulling in PartialEq for enum in public API.
    fn matches_direction(map: &FdMap, expected: StreamDirection) {
        match (&map.direction, expected) {
            (StreamDirection::ToChild, StreamDirection::ToChild) => {}
            (StreamDirection::FromChild, StreamDirection::FromChild) => {}
            _ => panic!("unexpected direction mapping: found {:?}, expected {:?}", map.direction, expected),
        }
    }

    fn assert_child_exit_ok(child: Pid) {
        match waitpid(child, None).expect("waitpid failed") {
            WaitStatus::Exited(_, code) => assert_eq!(code, 0, "child exited with {code}"),
            // Need to terminate the child on other wait statuses.
            // Otherwise, this could lead to zombie processes.
            ws => {
                unsafe { libc::kill(child.as_raw(), libc::SIGKILL) };
                panic!("unexpected wait status: {ws:?}")
            }
        }
    }

    fn exit_on_err<T>(res: Result<T, std::io::Error>) -> T {
        match res {
            Ok(val) => val,
            Err(_) => exit_with(1),
        }
    }

    fn exit_ok() -> ! {
        exit_with(0)
    }
    fn exit_with(code: i32) -> ! {
        unsafe { libc::_exit(code) }
    }
}
