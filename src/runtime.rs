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
    handler.handle(Box::new(child))?;
    state.child_exit_code()
}

#[cfg(target_os = "windows")]
mod spawn_windows;

#[cfg(target_os = "windows")]
pub fn sandbox_child<CH: CommHandler>(
    env: LaunchEnv,
    handler: CH,
) -> Result<i32, error::SandboxError> {
    todo!()
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
