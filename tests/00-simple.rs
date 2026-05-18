// SPDX-License-Identifier: MIT

//! The no-interaction executables tests.

use std::{path::PathBuf, sync::mpsc, thread};

use gracklezero::{
    FdSet, LaunchEnv, Restrictions, compat_restrictions, restrictions,
    runtime::{ExitCode, error::SandboxError},
    sandbox_child,
};
use tempfile::NamedTempFile;

mod common;
use common::{
    gen_r::{self, generate_restrictions},
    simple_handler, util,
};

use crate::common::simple_handler::TestMonitor;

/// Try running an executable that does not exist.
#[test]
fn not_exist() {
    fn empty_temp_path() -> PathBuf {
        let exec_file = NamedTempFile::new().expect("created a temp file");
        let name = &exec_file.path();
        PathBuf::from(name)
    }
    let path = empty_temp_path();
    assert!(!path.exists());
    let (h, m) = simple_handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: path,
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: util::std_fd(),
            restrictions: compat_restrictions!("noop"),
        },
        h,
    );
    m.assert_never_started();
    match &res {
        Ok(_) => {
            panic!("incorrectly returned from sandbox_child without an error");
        }
        Err(e) => {
            match e {
                // Some library code error; that's the expected result.  The
                // actual error is OS dependent.
                SandboxError::Io(_) => (),
                e => {
                    panic!("Invalid generated error: {:?}", e);
                }
            }
        }
    }
}

/// Perform no action with a minimal executable.
/// This ensures that, for a program that performs no offending operation,
/// with absolute minimal executable dependencies,
/// it runs and returns a zero exit code.
#[test]
fn simple_c() {
    for restr in generate_restrictions() {
        let (res, m) = run_simple_c(&restr.0, restr.1);
        res.expect("should have ran successfully");
        m.assert_exited_with(0);
        println!("Ran successfully: {}", restr.0)
    }
}

fn run_simple_c(
    name: &String,
    restr: Restrictions,
) -> (Result<ExitCode, SandboxError>, TestMonitor) {
    println!(
        "Running with restrictions {} + (always disable win32k disabled due to native hook issues)",
        &name
    );
    // This has the added condition for Windows where it *should* run with win2k dlls disabled.
    // However, some OS installations use things like virus scanners that can hook into the executed
    // programs and indirectly cause the win32k or gdi calls, which will trigger the execution to
    // fail to launch.  Therefore, this explicit flag is turned off.
    // restr.windows.disable_win32k_system_calls =
    //     gracklezero::restrictions::windows::AlwaysMode::AlwaysOn;

    let (h, m) = simple_handler::new();
    let res = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("simple-c"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: FdSet::basic(&[]),
            restrictions: restr,
        },
        h,
    );
    return (res, m);
}

/// Attempt to run the simple-c program in parallel to try to find
/// issues with the synchornization features necessary to prevent some
/// edge cases when starting multiple jailed programs close together.
#[test]
fn simple_c_parallel() {
    let (tx, rx) = mpsc::channel();
    let mut handles = vec![];
    for restr in generate_restrictions() {
        let tx_c = tx.clone();
        let handle = thread::spawn(move || {
            let restr_name = restr.0;
            let (res, m) = run_simple_c(&format!("parallel {}", &restr_name), restr.1);
            tx_c.send((restr_name, res, m))
        });
        handles.push(handle);
    }
    // Full error collection, for inspection of every thread run.
    let mut errors: Vec<String> = vec![];
    for handle in handles {
        match handle.join() {
            Ok(res) => match res {
                Ok(_) => (),
                Err(e) => {
                    errors.push(format!("send failure: {:?}", e));
                }
            },
            Err(e) => {
                errors.push(format!("thread failed: {:?}", e));
            }
        }
    }
    // Force the send channel closed, because all the handles completed.
    drop(tx);

    for (name, res, m) in rx.iter() {
        match res {
            Ok(_) => (),
            Err(e) => {
                errors.push(format!("{}: did not run successfully: {:?}", name, e));
            }
        };
        match &m.exit_code() {
            ExitCode::Exited(c) => {
                if *c != 0 {
                    errors.push(format!("{}: exited with {}, expected 0", name, *c));
                }
            }
            ExitCode::OsError(term) => {
                errors.push(format!("{}: terminated due to {:?}", name, term));
            }
            ExitCode::Running => {
                errors.push(format!("{}: still running after timeout", name));
            }
        }
    }
    let expected: Vec<String> = vec![];
    assert_eq!(expected, errors);
}

/// Perform no action with a minimal Rust executable.
/// This ensures that, for a program that performs no offending operation,
/// with absolute minimal executable dependencies,
/// it runs and returns a zero exit code.
#[test]
fn simple_rust() {
    for restr in generate_restrictions() {
        println!("Running with restrictions {}", restr.0);
        let (h, m) = simple_handler::new();
        sandbox_child(
            LaunchEnv {
                cmd: util::require_exec("simple-rust"),
                args: util::str_as_args("not used"),
                cwd: PathBuf::from("."),
                env: util::env_backtrace(),
                fds: FdSet::basic(&[]),
                restrictions: restr.1,
            },
            h,
        )
        .expect("should have ran successfully");
        m.assert_exited_with(0);
    }
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly runs.
#[cfg(target_os = "windows")]
#[test]
fn cfg_on_indirect() {
    run_cfg_on(
        "indirect",
        restrictions::windows::indirect_control_flow_guard,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions +
/// export suppression.
#[cfg(target_os = "windows")]
#[test]
fn cfg_on_export_suppression() {
    run_cfg_on(
        "export_suppression",
        restrictions::windows::control_flow_guard_export_suppression,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly runs.
#[cfg(target_os = "windows")]
#[test]
fn cfg_on_required() {
    run_cfg_on(
        "required",
        restrictions::windows::require_control_flow_guard,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly runs.
#[cfg(target_os = "windows")]
#[test]
fn cfg_on_strict() {
    run_cfg_on("strict", restrictions::windows::strict_control_flow_guard);
}

fn run_cfg_on(kind: &str, wrapper: fn(restrictions::Restrictions) -> restrictions::Restrictions) {
    let exec = match util::find_exec("simple-cfg") {
        None => {
            print!(
                "Skipping test; simple-cfg not found so assuming that the current system doesn't support compiling for it"
            );
            return;
        }
        Some(e) => e,
    };
    for restr in generate_restrictions() {
        println!("Running with {} CFG restrictions + {}", kind, restr.0);
        let cfg = wrapper(restr.1);
        let (h, m) = simple_handler::new();
        sandbox_child(
            LaunchEnv {
                cmd: exec.clone(),
                args: util::str_as_args("not used"),
                cwd: PathBuf::from("."),
                env: util::env_backtrace(),
                fds: FdSet::basic(&[]),
                restrictions: cfg,
            },
            h,
        )
        .expect("should have ran successfully");
        m.assert_exited_with(0);
    }
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly runs.
#[cfg(target_os = "windows")]
#[test]
fn cfg_off_indirect() {
    run_cfg_off_ok(
        "indirect",
        restrictions::windows::indirect_control_flow_guard,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions +
/// export suppression.  This should successfully run.
#[cfg(target_os = "windows")]
#[test]
fn cfg_off_export_suppression() {
    run_cfg_off_ok(
        "export_suppression",
        restrictions::windows::control_flow_guard_export_suppression,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly fails to run.
#[cfg(target_os = "windows")]
#[test]
fn cfg_off_required() {
    run_cfg_off_fail(
        "required",
        restrictions::windows::require_control_flow_guard,
    );
}

/// Same set of tests, but with Windows Control Flow Guard (CFG) restrictions.
/// This uses the executable without the CFG compiled executable, to ensure
/// that it correctly fails to run.
#[cfg(target_os = "windows")]
#[test]
fn cfg_off_strict() {
    run_cfg_off_fail("strict", restrictions::windows::strict_control_flow_guard);
}

fn run_cfg_off_ok(
    kind: &str,
    wrapper: fn(restrictions::Restrictions) -> restrictions::Restrictions,
) {
    let restr = wrapper(gen_r::desktop_restrictions());
    println!("Running with restrictions {}", kind);
    let (h, m) = simple_handler::new();
    let _ = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("simple-c"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: FdSet::basic(&[]),
            restrictions: restr,
        },
        h,
    )
    .expect("started with an error");
    m.assert_exited_with(0);
}

fn run_cfg_off_fail(
    kind: &str,
    wrapper: fn(restrictions::Restrictions) -> restrictions::Restrictions,
) {
    let restr = wrapper(gen_r::desktop_restrictions());
    println!("Running with restrictions {}", kind);
    let (h, m) = simple_handler::new();
    let err = sandbox_child(
        LaunchEnv {
            cmd: util::require_exec("simple-c"),
            args: util::str_as_args("not used"),
            cwd: PathBuf::from("."),
            env: util::env_backtrace(),
            fds: FdSet::basic(&[]),
            restrictions: restr,
        },
        h,
    )
    .expect_err("did not exit with error");
    m.assert_never_started();
    assert!(
        format!("{:?}", err).contains("image file was blocked from loading"),
        "Unexpected error message: {:?}",
        err,
    );
}
