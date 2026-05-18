// SPDX-License-Identifier: MIT

use crate::runtime::spawn::{ExitCode, OsTermination};

use super::jail::ProcessInfo;
use std::{
    ptr::null,
    sync::{Arc, Mutex},
};
use windows::{
    Win32::{
        Foundation::{self, CloseHandle, HANDLE},
        System::{
            Diagnostics, JobObjects::TerminateJobObject, LibraryLoader,
            Threading::GetExitCodeProcess,
        },
    },
    core,
};

/// Allows monitoring the state of the launched process.
#[derive(Clone)]
pub struct ProcessState {
    mutable: Arc<Mutex<MutableProcessState>>,

    // idiomatic Rust would have this be an option, to deal with drop() support.
    info: ProcessInfo,
}

impl ProcessState {
    pub fn new(info: ProcessInfo) -> Self {
        Self {
            mutable: Arc::new(Mutex::new(MutableProcessState {
                terminated: false,
                exit_code: None,
            })),
            info,
        }
    }

    /// Terminate the process.
    /// This will only send the termination once.
    /// Need to investigate whether situations may arise where it may be necessary to run this
    /// multiple times on the same process.
    pub fn terminate(&self, exit_code: u32) -> Result<(), std::io::Error> {
        let mut guard = self
            .mutable
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        if !(*guard).terminated {
            self.inner_terminate(exit_code)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Interrupted, e))?;
            (*guard).terminated = true;
        }
        Ok(())
    }

    fn inner_terminate(&self, exit_code: u32) -> core::Result<()> {
        unsafe {
            // TerminateJobObject kills everything in the job.
            // A very reliable way to stop it, if the process somehow found a way to break out
            // and spawn its own process.
            TerminateJobObject(self.info.job, exit_code)
        }
    }

    /// Get the exit code for the process, or None if it hasn't exited yet.
    pub fn exit_code(&self) -> Result<ExitCode, std::io::Error> {
        let mut guard = self
            .mutable
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        match &(*guard).exit_code {
            Some(c) => Ok(c.clone()),
            None => {
                let code = self
                    .inner_exit_code()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Interrupted, e))?;
                match &code {
                    // Don't capture the Running state.
                    ExitCode::Running => (),
                    _ => {
                        (*guard).exit_code = Some(code.clone());
                    }
                }
                Ok(code)
            }
        }
    }

    fn inner_exit_code(&self) -> core::Result<ExitCode> {
        unsafe {
            let mut code = 0u32;
            GetExitCodeProcess(self.info.process, &mut code)?;
            Ok(Self::from_code(code))
        }
    }

    fn from_code(code: u32) -> ExitCode {
        let mut buffer: Vec<u16> = vec![0; FORMAT_MESSAGE_BUFFER_SIZE];
        let m_null: *mut ::core::ffi::c_void = null::<()>() as *mut ::core::ffi::c_void;

        if code > i32::MAX as u32 {
            // Try ntdll.dll, as that stores these larger exit codes.
            unsafe {
                // Try ntdll.dll first (direct NTSTATUS lookup)
                let module: Foundation::HMODULE = match LibraryLoader::GetModuleHandleW(
                    core::PCWSTR(windows::core::w!("ntdll.dll").as_ptr()),
                ) {
                    Ok(m) => m,
                    Err(_) => Foundation::HMODULE(m_null),
                };

                if module.0 != m_null {
                    let len = Diagnostics::Debug::FormatMessageW(
                        Diagnostics::Debug::FORMAT_MESSAGE_FROM_HMODULE
                            | Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS,
                        Some(module.0 as *const ::core::ffi::c_void),
                        code,
                        0,
                        core::PWSTR(buffer.as_mut_ptr()),
                        FORMAT_MESSAGE_BUFFER_SIZE as u32,
                        None,
                    );
                    if len > 0 {
                        return ExitCode::OsError(OsTermination {
                            message: String::from_utf16_lossy(&buffer[..len as usize])
                                .trim()
                                .to_string(),
                            code: code as i64,
                            subcode: None,
                        });
                    }
                }
            }
            return ExitCode::OsError(OsTermination {
                message: format!("Process failed with OS error code 0x{:X}", code),
                code: code as i64,
                subcode: None,
            });
        }

        // Everything else deals with a 32 bit exit code.

        let icode = code as i32;
        if icode == Foundation::STILL_ACTIVE.0 {
            return ExitCode::Running;
        }
        if icode < Foundation::STILL_ACTIVE.0 {
            return ExitCode::Exited(icode);
        }

        // Try NTSTATUS as a Win32 error
        let win32_err = unsafe { Foundation::RtlNtStatusToDosError(Foundation::NTSTATUS(icode)) };

        let len = unsafe {
            Diagnostics::Debug::FormatMessageW(
                Diagnostics::Debug::FORMAT_MESSAGE_FROM_SYSTEM
                    | Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS,
                None,
                win32_err,
                0,
                core::PWSTR(buffer.as_mut_ptr()),
                FORMAT_MESSAGE_BUFFER_SIZE as u32,
                None,
            )
        };
        if len > 0 {
            return ExitCode::OsError(OsTermination {
                message: String::from_utf16_lossy(&buffer[..len as usize])
                    .trim()
                    .to_string(),
                code: icode as i64,
                subcode: Some(win32_err as i64),
            });
        }

        // If all else fails, return the raw code.
        return ExitCode::OsError(OsTermination {
            message: format!("Unknown error: 0x{:08X}", icode),
            code: icode as i64,
            subcode: Some(win32_err as i64),
        });
    }
}

// Max message size recommended by Microsoft docs
const FORMAT_MESSAGE_BUFFER_SIZE: usize = 2048;

impl Drop for ProcessState {
    fn drop(&mut self) {
        // Note: ignoring errors inside the drop.

        // Ensure it's been killed.
        let _ = self.terminate(255);

        // Close off handles.
        unsafe {
            if self.info.thread != HANDLE(std::ptr::null_mut()) {
                let _ = CloseHandle(self.info.thread);
                self.info.thread = HANDLE(std::ptr::null_mut());
            }
            if self.info.process != HANDLE(std::ptr::null_mut()) {
                let _ = CloseHandle(self.info.process);
                self.info.process = HANDLE(std::ptr::null_mut());
            }
            if self.info.job != HANDLE(std::ptr::null_mut()) {
                let _ = CloseHandle(self.info.job);
                self.info.job = HANDLE(std::ptr::null_mut());
            }
        }
    }
}

struct MutableProcessState {
    terminated: bool,
    exit_code: Option<ExitCode>,
}
