//! Utility helpers for running the tests.

use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{FdMode, FdSet};

/// Convert the path value into an array of arguments.
pub fn path_as_args(path: &Path) -> Vec<OsString> {
    vec![path.into()]
}

/// Convert the string value into an array of arguments.
pub fn str_as_args(s: &str) -> Vec<OsString> {
    vec![OsString::from(s)]
}

/// Convert the string value into an array of arguments.
pub fn string_as_args(s: &String) -> Vec<OsString> {
    vec![OsString::from(s)]
}

/// Create the standard FD set used for integration tests.
/// This tells the runtime execution to construct in the child:
///     stdin (0): a stream that writes to the child.
///    stdout (1): a stream that reads from the child.
///    stderr (2): pipe from the child directly into the executing test's stderr.
pub fn std_fd() -> FdSet {
    FdSet::basic(&[FdMode::ToChild, FdMode::FromChild, FdMode::KeepInChild])
}

#[cfg(target_os = "windows")]
const EXEC_SUFFIX: &str = ".exe";

#[cfg(not(target_os = "windows"))]
const EXEC_SUFFIX: &str = "";

/// Find the executable for the given test program.
pub fn find_exec(exec_name: &str) -> PathBuf {
    // Find the 'tests' directory off the root.
    let test_dir = Path::new("tests");
    assert!(test_dir.is_dir());

    // Slowly build up the path, bit by bit.
    // This allows for more robust error messages to help the
    // developer out.
    let mut exec: PathBuf = test_dir.into();
    exec.push(exec_name);
    let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
    assert!(exec.is_dir(), "did not find test directory ({})?", exec_s);
    exec.push("target");
    let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
    assert!(
        exec.is_dir(),
        "could not find {}; did you remember to run 'cargo build' on it?",
        exec_s,
    );
    exec.push("debug");
    let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
    assert!(
        exec.is_dir(),
        "could not find {}; did you remember to run 'cargo build' on it?",
        exec_s,
    );
    exec.push(format!("{exec_name}{EXEC_SUFFIX}"));
    let exec_s: String = exec.as_os_str().to_string_lossy().to_string();
    assert!(
        exec.is_file(),
        "could not find {}; did you remember to run 'cargo build' on it?",
        exec_s,
    );
    exec
}

/// Create an environment that tells the executed rust program to include the backtrace.
pub fn env_backtrace() -> HashMap<OsString, OsString> {
    let mut env = HashMap::new();
    env.insert(OsString::from("RUST_BACKTRACE"), OsString::from("1"));
    env
}
