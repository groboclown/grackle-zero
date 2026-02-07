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
    let err = handler.handle(Box::new(child));
    let ret = state.kill().map_err(|e| e.into());
    err?;
    ret
}

#[cfg(target_os = "windows")]
mod spawn_windows;

#[cfg(target_os = "windows")]
pub fn sandbox_child<CH: CommHandler>(
    env: LaunchEnv,
    handler: CH,
) -> Result<i32, error::SandboxError> {
    let child = spawn_windows::launch_child(env)?;
    let state = child.state();
    // dropping the child object will kill the child process and all the open handles.
    let err = handler.handle(Box::new(child));
    // force termination if the handler didn't and instead quit with an error.
    let ret = match state.exit_code()? {
        Some(v) => Ok(v as i32),
        None => Err(error::SandboxError::ProcessError("did not exit cleanly".to_string()))
    };
    err?;
    ret
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
