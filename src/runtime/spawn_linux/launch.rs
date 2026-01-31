// SPDX-License-Identifier: MIT

//! Launch the child process.

use std::{collections::{HashMap, HashSet}, ffi::CString, os::unix::ffi::OsStrExt as _, path::PathBuf};

use nix::sys::wait::WaitStatus;

use crate::runtime::{
    error::SandboxError,
    spawn::{Child, LaunchEnv},
    spawn_linux::{
        dependencies::find_bin_dependencies,
        fd::{FdMap, ForkedFd, StreamDirection},
        jail::LandlockJail,
    },
};

pub struct LinuxChild {
    pid: nix::unistd::Pid,
    fds: HashMap<u32, FdMap>,
}

impl LinuxChild {
    pub(crate) fn state(&self) -> LinuxChildState {
        LinuxChildState { pid: self.pid }
    }
}

/// Handle the child process launching.
pub fn launch_child(env: LaunchEnv) -> Result<LinuxChild, SandboxError> {
    // As much as possible is performed before the fork.
    // That's because, according to the fork docs:
    //
    // > In a multithreaded program, only [async-signal-safe] functions like `pause`
    // > and `_exit` may be called by the child (the parent isn't restricted) until
    // > a call of `execve(2)`. Note that memory allocation may **not** be
    // > async-signal-safe and thus must be prevented.
    let exec_path = which::which(&env.cmd)?;
    let sandbox = LandlockJail::new(&extract_dependencies(find_bin_dependencies(&exec_path))?)?;
    let fd_set = ForkedFd::new(env.fds)?;
    let exec_path = CString::new(exec_path.as_os_str().as_bytes())?;
    let exec_path = exec_path.as_c_str();
    let cwd = CString::new(env.cwd.as_os_str().as_bytes())?;
    let cwd = cwd.as_c_str();
    let mut args = Vec::new();
    for arg in env.args {
        args.push(CString::new(arg.as_os_str().as_bytes())?);
    }
    let args = args.as_slice();
    let mut environ = Vec::new();
    for (key, val) in env.env.iter() {
        let mut entry = key.clone();
        entry.push("=");
        entry.push(val);
        environ.push(CString::new(entry.as_os_str().as_bytes())?);
    }
    let environ = environ.as_slice();
    let child_fds = fd_set.child_fd_list();

    match unsafe { nix::unistd::fork() } {
        Err(e) => Err(SandboxError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e,
        ))),
        Ok(nix::unistd::ForkResult::Child) => {
            // Any errors in here must trigger an immediate exit.
            // Anything that runs here can't allocate memory.
            fd_set.child_after_fork();

            // This looks like it just creates data in the stack, not allocated
            // on the heap, which means it's fine to call.
            if nix::unistd::chdir(cwd).is_err() {
                std::process::exit(253);
            }
            sandbox.restrict();

            // Because the landlock uses a FD under the hood, the child FDs must be
            // closed after calling restrict.
            close_open_fds(&child_fds);

            // Run the executable.
            let _ = nix::unistd::execve(exec_path, args, environ);
            // To reach here means the exec failed.
            std::process::exit(254);
        }
        Ok(nix::unistd::ForkResult::Parent { child }) => {
            let fds = fd_set.parent_after_fork();
            Ok(LinuxChild {
                pid: child,
                fds: fd_map(fds),
            })
        }
    }
}

impl Child for LinuxChild {
    fn terminate(&self) -> Result<(), std::io::Error> {
        // The child cannot listen to signals, so kill it hard.
        nix::sys::signal::kill(self.pid, nix::sys::signal::Signal::SIGKILL)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn take_stream_from_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Read>> {
        match self.fds.remove(&fd) {
            Some(fd) => match fd.direction {
                StreamDirection::FromChild => Some(Box::new(fd.stream)),
                _ => None,
            },
            None => None,
        }
    }

    fn take_stream_to_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Write>> {
        match self.fds.remove(&fd) {
            Some(fd) => match fd.direction {
                StreamDirection::ToChild => Some(Box::new(fd.stream)),
                _ => None,
            },
            None => None,
        }
    }

    fn exit_status(&self) -> Option<i32> {
        match nix::sys::wait::waitpid(
            self.pid,
            nix::sys::wait::WaitPidFlag::from_bits(nix::libc::WNOHANG),
        ) {
            // An error usually means that the child never started.
            Err(_) => None,
            Ok(WaitStatus::Exited(_pid, c)) => Some(c),
            Ok(WaitStatus::StillAlive) => None,
            Ok(WaitStatus::Signaled(_, _, _)) => None,
            Ok(WaitStatus::Stopped(_, _)) => None,
            Ok(WaitStatus::PtraceEvent(_, _, _)) => None,
            Ok(WaitStatus::PtraceSyscall(_)) => None,
            Ok(WaitStatus::Continued(_)) => None,
        }
    }
}

fn extract_dependencies(
    deps: Vec<super::dependencies::Dependency>,
) -> Result<Vec<PathBuf>, SandboxError> {
    let mut is_ok = true;
    let mut missing = String::new();
    let mut ret = Vec::new();
    for dep in deps {
        if dep.invalid() {
            if is_ok {
                is_ok = false;
            } else {
                missing.push_str(", ");
            }
            missing.push_str(
                dep.best_path()
                    .as_os_str()
                    .to_string_lossy()
                    .to_string()
                    .as_str(),
            );
        } else if dep.exists() {
            ret.push(dep.best_path().clone());
        } // else ignore
    }
    if is_ok {
        Ok(ret)
    } else {
        Err(SandboxError::JailSetup(format!(
            "missing library dependencies: {missing}"
        )))
    }
}

fn fd_map(src: Vec<FdMap>) -> HashMap<u32, FdMap> {
    let mut ret = HashMap::new();
    for f in src {
        ret.insert(f.dup_to, f);
    }
    ret
}

/// Close all open file descriptors except those listed.
/// This method may be imperfect if
/// the system has a very high limit on open FDs.
/// 
/// Another method would have this look in /proc/self/fd, but that
/// would allocate memory, unless this takes extreme care using low-level
/// libc calls.  Additionally, that would need to read from the file system,
/// which the landlock may have blocked, and, reading before the restriction
/// would lead to closing off the landlocks' owned file descriptor.
fn close_open_fds(except: &HashSet<nix::libc::c_int>) {
    let max_fd = match nix::unistd::sysconf(nix::unistd::SysconfVar::OPEN_MAX) {
        Ok(Some(n)) => n as nix::libc::c_int,
        _ => 1024,
    };
    for fd in 0..max_fd as nix::libc::c_int {
        if !except.contains(&fd) {
            // Ignore errors, in case the FD is already closed.
            // Also, it skips going through the nix::* layers, which may allocate memory.
            let _ = unsafe { nix::libc::close(fd) };
        }
    }
}

/// Structure that allows querying the state of a launched Linux child process,
/// outside the CallHandler use.
pub (crate) struct LinuxChildState {
    pid: nix::unistd::Pid,
}

impl LinuxChildState {
    pub (crate) fn child_exit_code(&self) -> Result<i32, SandboxError> {
        match nix::sys::wait::waitpid(
            self.pid,
            nix::sys::wait::WaitPidFlag::from_bits(nix::libc::WNOHANG),
        ) {
            // An error usually means that the child never started.
            Err(r) => Err(SandboxError::ProcessError(r.to_string())),
            Ok(WaitStatus::Exited(_pid, c)) => Ok(c),
            Ok(_) => {
                // Still alive, so need to kill it.
                nix::sys::signal::kill(self.pid, nix::sys::signal::Signal::SIGKILL)
                    .map_err(|e| SandboxError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                // Call and wait for the termination this time.
                match nix::sys::wait::waitpid(self.pid, None) {
                    Err(r) => Err(SandboxError::ProcessError(r.to_string())),
                    Ok(WaitStatus::Exited(_pid, c)) => Ok(c),
                    Ok(_) => Err(SandboxError::ProcessError(
                        "unexpected wait status after killing child".to_string(),
                    )),
                }
            }
        }
    }
}
