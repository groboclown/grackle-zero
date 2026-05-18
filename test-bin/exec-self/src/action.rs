// SPDX-License-Identifier: MIT

use super::debug::debug;
use std::process::{Command, Stdio};

pub(crate) fn perform(arg: String) {
    if arg == "sub" {
        debug(format!("Successfully running child program"));
        return;
    }
    debug(format!("Running self as child command"));
    let current_exe = std::env::current_exe().unwrap();
    let out = Command::new(current_exe)
        .arg("sub")
        .stderr(Stdio::inherit())
        .output()
        .unwrap();
    debug(std::string::String::from_utf8(out.stdout).unwrap());
}
