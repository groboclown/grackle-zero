//! Run the process in such a way that it can used by the spawn handler.


use std::{collections::HashMap, ffi::{OsStr, OsString}, path::PathBuf};

use windows::Win32::Foundation::HANDLE;

use crate::{FdSet, runtime::{
    error::SandboxError,
    spawn::{Child, LaunchEnv},
    spawn_windows::{
        fd::{StdIo, StdIoFd, StdIoSet, StreamDirection, WinFd, WinFdSet},
        jail, launch_quote, monitor::ProcessState
    },
}};

pub struct WindowsChild {
    state: ProcessState,
    stdin: Option<StdIoFd>,
    stdout: Option<StdIoFd>,
    stderr: Option<StdIoFd>,
    others: HashMap<u32, WinFd>,
}

const LAUNCH_HANDLE_ENV: &str = "SANDBOX_HANDLES";


/// Handle the child process launching.
pub fn launch_child(mut env: LaunchEnv) -> Result<WindowsChild, SandboxError> {
    let cmd = get_full_path_name(&env.cmd)?;  // must be a real path, not a relative location.
    let args = launch_quote::quote_arguments(OsStr::new("command.com"), &env.args)?; // use a placeholder cmd name
    let (fds, handles, env_handles) = create_fds(env.fds)?;
    let cwd = get_full_path_name(&env.cwd)?; // Must be a real path, not a relative location.
    println!("Running [{}] [{}] in [{}]", cmd.to_str().unwrap(),  String::from_utf16(args.as_slice()).unwrap(), cwd.to_str().unwrap());

    let mut environ: Vec<(OsString, OsString)> = env.env.drain().collect();
    environ.push((OsString::from(LAUNCH_HANDLE_ENV), env_handles));
    let environ = launch_quote::encode_env_strings(environ.as_slice())?;

    let child = jail::launch_restricted(
        cmd.as_os_str(),
        &args,
        cwd.as_os_str(),
        environ,
        match &fds.stdin {
            StdIoFd::None => None,
            StdIoFd::Pipe(v) => v.child(),
        },
        match &fds.stdout {
            StdIoFd::None => None,
            StdIoFd::Pipe(v) => v.child(),
        },
        match &fds.stderr {
            StdIoFd::None => None,
            StdIoFd::Pipe(v) => v.child(),
        },
        handles.as_slice(),
    )
        .map_err(|e| SandboxError::JailSetup(format!("problem launching process: {:?}", e)))?;

    Ok(WindowsChild::new(child, fds))
}


impl WindowsChild {
    fn new(proc: jail::ProcessInfo, fds: WinFdSet) -> Self {
        let mut others = HashMap::new();
        for fd in fds.others {
            others.insert(fd.fd(), fd);
        }

        WindowsChild {
            state: ProcessState::new(proc),
            stdin: Some(fds.stdin),
            stdout: Some(fds.stdout),
            stderr: Some(fds.stderr),
            others,
        }
    }


    pub(crate) fn state(&self) -> ProcessState {
        self.state.clone()
    }
}


impl Child for WindowsChild {
    fn terminate(&self) -> Result<(), std::io::Error> {
        self.state.terminate(255)
    }

    fn take_stream_from_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Read>> {
        match fd {
            0 => None,  // stdin is a parent writer, not a reader.
            1 => match self.stdout.take() {
                None => None,
                Some(s) => match s {
                    StdIoFd::None => None,
                    StdIoFd::Pipe(mut v) => v.as_reader(),
                }
            }
            2 => match self.stderr.take() {
                None => None,
                Some(s) => match s {
                    StdIoFd::None => None,
                    StdIoFd::Pipe(mut v) => v.as_reader(),
                }
            }
            fd => {
                match self.others.remove(&fd) {
                    None => None,
                    Some(mut v) => v.as_reader(),
                }
            }
        }
    }

    fn take_stream_to_child(&mut self, fd: u32) -> Option<Box<dyn std::io::Write>> {
        match fd {
            0 => match self.stdin.take() {
                None => None,
                Some(s) => match s {
                    StdIoFd::None => None,
                    StdIoFd::Pipe(mut v) => v.as_writer(),
                }
            }
            1 => None, // stdout is a parent reader, not writer
            2 => None, // stderr is a parent reader, not writer
            fd => {
                match self.others.remove(&fd) {
                    None => None,
                    Some(mut v) => v.as_writer(),
                }
            }
        }
    }

    fn exit_status(&self) -> Option<i32> {
        match self.state.exit_code() {
            Ok(v) => v.map(|c| c as i32),
            Err(_) => None,
        }
    }
}


fn create_fds(src: FdSet) -> Result<(WinFdSet, Vec<HANDLE>, OsString), SandboxError> {
    let mut stdin = StdIo::None;
    let mut stdout = StdIo::None;
    let mut stderr = StdIo::None;
    let mut others = vec![];

    for fd in src.modes() {
        match fd.fd {
            0 => {
                stdin = match fd.mode {
                    crate::FdMode::FromChild => { return Err(SandboxError::JailSetup("stdio marked as read from child".to_string())); }
                    crate::FdMode::Null => StdIo::None,
                    crate::FdMode::KeepInChild => StdIo::PassThrough,
                    crate::FdMode::ToChild => StdIo::Pipe,
                };
            }
            1 => {
                stdout = match fd.mode {
                    crate::FdMode::FromChild => StdIo::Pipe,
                    crate::FdMode::Null => StdIo::None,
                    crate::FdMode::KeepInChild => StdIo::PassThrough,
                    crate::FdMode::ToChild => { return Err(SandboxError::JailSetup("stdout marked as write to child".to_string())); }
                }
            }
            2 => {
                stderr = match fd.mode {
                    crate::FdMode::FromChild => StdIo::Pipe,
                    crate::FdMode::Null => StdIo::None,
                    crate::FdMode::KeepInChild => StdIo::PassThrough,
                    crate::FdMode::ToChild => { return Err(SandboxError::JailSetup("stdout marked as write to child".to_string())); }
                }
            }
            _ => {
                match fd.mode {
                    crate::FdMode::Null => (),
                    crate::FdMode::KeepInChild => {
                        return Err(SandboxError::JailSetup("windows cannot pass-through arbitrary handles".to_string()));
                    },
                    crate::FdMode::ToChild => {
                        others.push(WinFd::new(fd.fd, StreamDirection::ToChild)
                            .map_err(|e| SandboxError::JailSetup(format!("problem setting up fd: {:?}", e)))?);
                    }
                    crate::FdMode::FromChild => {
                        others.push(WinFd::new(fd.fd, StreamDirection::FromChild)
                            .map_err(|e| SandboxError::JailSetup(format!("problem setting up fd: {:?}", e)))?);
                    }

                }
            }
        };
    }

    let mut handles = vec![];
    let mut env_handles = OsString::new();
    for fd in &others {
        match fd.child() {
            None => (),
            Some(v) => { handles.push(v); }
        }
        match fd.as_env_val() {
            None => (),
            Some(v) => {
                env_handles.push(v);
            }
        }
    }
    Ok((
        WinFdSet::new(StdIoSet { stdin, stdout, stderr }, others)
            .map_err(|e| SandboxError::JailSetup(format!("problem setting up fd: {:?}", e)))?,
        handles,
        env_handles,
    ))
}


// Get the canonical Win32 path, not the extended-length path that canonicalize() generates.
fn get_full_path_name(path: &PathBuf) -> Result<PathBuf, std::io::Error> {
    // The "correct" way is to use GetFullPathNameW.  That's messy.
    // Future people may do it the right way.
    // For now, strip off the \\?\ that the extended-length path adds.
    let path = path.canonicalize()?;
    let path = path.as_os_str().as_encoded_bytes();
    if &path[0..4] == br"\\?\" as &[u8] {
        Ok(PathBuf::from(unsafe { OsStr::from_encoded_bytes_unchecked(&path[4..]) }))
    } else {
        Ok(PathBuf::from(unsafe { OsStr::from_encoded_bytes_unchecked(path) }))
    }
}
