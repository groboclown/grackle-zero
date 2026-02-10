// SPDX-License-Identifier: MIT

//! Wrapper for the AppContainer work.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows::Win32::Security::{self, Isolation};
use super::error::WindowsSandboxError;
use super::conv::as_c_str_w;

pub struct AppContainer {
    // name: String,
    // uid: String,
    sid: Option<Security::PSID>,
}


impl AppContainer {
    pub fn new(name: &str) -> Result<Self, WindowsSandboxError> {
        let appcontainer_name = as_c_str_w(&OsString::from(name));
        let mut uid = name.to_string();
        let mut idx = -1;
        loop {
            match unsafe { Security::Isolation::CreateAppContainerProfile(
                windows::core::PCWSTR(as_c_str_w(&OsString::from(uid)).as_ptr()),  // pszAppContainerName: identifies the container profile
                windows::core::PCWSTR(appcontainer_name.as_ptr()),                      // pszDisplayName: human-readable
                windows::core::PCWSTR(appcontainer_name.as_ptr()),                      // pszDescription
                None,                                                                    // pCapabilities: none (== no capabilities)
            ) } {
                Ok(sid) => {
                    return Ok(Self {
                        sid: Some(sid),
                    });
                }
                Err(e) => {
                    if e.code() != ERROR_ALREADY_EXISTS.into() {
                        return Err(WindowsSandboxError::Setup(e))
                    }
                    // If it already exists, fall through and try another AppContainer name.
                    // Could use DeriveAppContainerSidFromAppContainerName to reuse the other,
                    // but that could be dropped separately.  So create a new one to ensure it's managed by itself.
                }
            }

            idx += 1;
            uid = format!("{}{}", name, idx);
        }
    }

    pub fn sid(&self) -> Option<Security::PSID> {
        self.sid
    }

    fn sid_str(&self) -> Result<OsString, WindowsSandboxError> {
        let sid = match self.sid {
            None => return Err(WindowsSandboxError::setup_message("AppContainer already dropped")),
            Some(sid) => sid,
        };
        // This just really needs low-level access, as the windows crate doesn't provide it.
        // Windows will create the string in a buffer that this then needs to free.
        let mut buf_ptr: *mut u16 = std::ptr::null_mut();
        let result = unsafe {
            winapi::shared::sddl::ConvertSidToStringSidW(sid.0 as _, &mut buf_ptr)
        };

        if result == 0 {
            // No need to clean up
            Err(WindowsSandboxError::Sandbox(std::io::Error::last_os_error().into()))
        } else {
            // Create the OsString from the buffer, which is a null-terminated UTF-16 string,
            // and be sure to clean up the buffer.
            let mut nul_pos = 0;
            while unsafe { *buf_ptr.add(nul_pos) } != 0 {
                nul_pos += 1;
            }
            let slice_with_nul = unsafe { std::slice::from_raw_parts(buf_ptr, nul_pos + 1) };

            let os_string = OsString::from_wide(
                &slice_with_nul.iter()
                    .cloned()
                    .take_while(|&n| n != 0)
                    .collect::<Vec<u16>>(),
            );

            unsafe { winapi::um::winbase::LocalFree(buf_ptr as *mut _) };

            Ok(os_string)
        }
    }

    pub fn folder_path(&self) -> Result<String, WindowsSandboxError> {
        let sid = self.sid_str()?;
        let mut sid = as_c_str_w(&sid);
        unsafe {
            Isolation::GetAppContainerFolderPath(windows::core::PWSTR(sid.as_mut_ptr()))
                .map_err(|e| WindowsSandboxError::setup(e))?
                .to_string()
                .map_err(|e| WindowsSandboxError::setup(e.into()))
        }
    }
}


impl Drop for AppContainer {
    fn drop(&mut self) {
        match self.sid.take() {
            None => (),
            Some(sid) => {
                let _ = unsafe { Security::FreeSid(sid) };
            }
        }
    }
}
