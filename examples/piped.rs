// SPDX-License-Identifier: MIT

//! Just pipes stdout and stderr for a child process through the sandbox.
//! It reuses the current working directory and environment variables.
//!
//! Due to the way the sandbox runs, if you use a program like "busybox"
//! that relies on the first argument to determine the behavior, then it
//! won't work as expected, because the sandbox uses a placeholder name.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use gracklezero::{self, FdMode, strict_restrictions};

pub fn main() {
    let res = gracklezero::sandbox_child(
        gracklezero::LaunchEnv {
            cmd: get_cmd(),
            args: get_args(),
            cwd: std::env::current_dir().expect("failed to get current directory"),
            env: get_env(),
            fds: gracklezero::FdSet::basic(&[
                FdMode::Null,
                FdMode::KeepInChild,
                FdMode::KeepInChild,
            ]),
            restrictions: strict_restrictions!("piped"),
        },
        WaitHandler {},
    )
    .expect("Failed to run the child");
    match res {
        gracklezero::runtime::ExitCode::Exited(code) => {
            std::process::exit(code);
        }
        gracklezero::runtime::ExitCode::OsError(s) => {
            println!("Child exited with OS error: {} (0x{:X})", s.message, s.code);
            std::process::exit(100);
        }
        gracklezero::runtime::ExitCode::Running => {
            println!("Child is still running (this should not happen)");
            std::process::exit(101);
        }
    }
}

fn get_cmd() -> PathBuf {
    std::env::args()
        .nth(1)
        .expect("missing argument.  First argument is the command to run.")
        .into()
}

fn get_args() -> Vec<OsString> {
    std::env::args_os().skip(2).collect()
}

fn get_env() -> HashMap<OsString, OsString> {
    std::env::vars_os().collect()
}

struct WaitHandler {}

impl gracklezero::CommHandler for WaitHandler {
    fn handle(self, child: Box<dyn gracklezero::Child>) -> Result<(), std::io::Error> {
        loop {
            match child.exit_status() {
                gracklezero::runtime::ExitCode::Exited(code) => {
                    println!("Child exited with code: {}", code);
                    return Ok(());
                }
                gracklezero::runtime::ExitCode::OsError(s) => {
                    println!("Child exited with OS error: {} (0x{:X})", s.message, s.code);
                    return Ok(());
                }
                gracklezero::runtime::ExitCode::Running => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
