// SPDX-License-Identifier: MIT

use std::{
    ffi::OsString,
    hash::{Hash, Hasher},
};

use windows::Win32::{Foundation, System::Threading};

use super::conv::as_c_str_w;
use super::error::WindowsSandboxError;

/// An OS-wide lock for creating the new AppContainer
pub struct OsLock {
    handle: Foundation::HANDLE,
}

impl OsLock {
    pub fn acquire(name: &str) -> Result<Self, WindowsSandboxError> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let lock_name = format!(
            "Local\\gracklezero-appcontainer-init-{:016x}",
            hasher.finish()
        );
        let lock_name_w = as_c_str_w(&OsString::from(lock_name));
        let handle = unsafe {
            Threading::CreateMutexW(None, false, windows::core::PCWSTR(lock_name_w.as_ptr()))
                .map_err(WindowsSandboxError::setup)?
        };
        match unsafe { Threading::WaitForSingleObject(handle, 30_000) } {
            Foundation::WAIT_OBJECT_0 | Foundation::WAIT_ABANDONED => Ok(Self { handle }),
            Foundation::WAIT_TIMEOUT => {
                unsafe {
                    let _ = Foundation::CloseHandle(handle);
                }
                Err(WindowsSandboxError::setup_message(
                    "timed out waiting for AppContainer initialization lock",
                ))
            }
            _ => {
                unsafe {
                    let _ = Foundation::CloseHandle(handle);
                }
                Err(WindowsSandboxError::setup_message(
                    "failed waiting for AppContainer initialization lock",
                ))
            }
        }
    }
}

impl Drop for OsLock {
    fn drop(&mut self) {
        unsafe {
            let _ = Threading::ReleaseMutex(self.handle);
            let _ = Foundation::CloseHandle(self.handle);
        }
    }
}
