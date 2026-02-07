// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, STILL_ACTIVE},
        System::{JobObjects::TerminateJobObject, Threading::GetExitCodeProcess},
    },
    core,
};
use super::jail::ProcessInfo;


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
    pub fn exit_code(&self) -> Result<Option<u32>, std::io::Error> {
        let mut guard = self
            .mutable
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        match (*guard).exit_code {
            Some(c) => Ok(Some(c)),
            None => {
                let code = self.inner_exit_code()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Interrupted, e))?;
                (*guard).exit_code = code.clone();
                Ok(code)
            }
        }
    }

    fn inner_exit_code(&self) -> core::Result<Option<u32>> {
        unsafe {
            let mut code = 0u32;
            GetExitCodeProcess(self.info.process, &mut code)?;

            if code == STILL_ACTIVE.0 as u32 {
                Ok(None)
            } else {
                Ok(Some(code))
            }
        }
    }
}


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
    exit_code: Option<u32>,
}
