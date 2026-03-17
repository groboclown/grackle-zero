// SPDX-License-Identifier: MIT

//! Wrapper for the AppContainer work.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use super::conv::as_c_str_w;
use super::error::WindowsSandboxError;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, HLOCAL, LocalFree};
use windows::Win32::Security;

pub struct AppContainer {
    // name: String,
    // uid: String,
    sid: Option<Security::PSID>,
    _sa: Security::SECURITY_ATTRIBUTES,
    _sd: Vec<u8>,
    acl: Option<std::ffi::c_void>,
}

impl AppContainer {
    // TODO take in the restrictions list.

    /// Create or retrieve the new app container.
    /// Note: the returned app container is persistent across executions, per user.
    pub fn new(name: &str) -> Result<Self, WindowsSandboxError> {

        let appcontainer_name = as_c_str_w(&OsString::from(name));
        let mut uid = name.to_string();
        let mut idx = -1;
        loop {
            // TODO add in WinAppContainerCapability capabilities.
            // See https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer?source=recommendations
            // for how to do this.

            match unsafe {
                Security::Isolation::CreateAppContainerProfile(
                    windows::core::PCWSTR(as_c_str_w(&OsString::from(uid)).as_ptr()), // pszAppContainerName: identifies the container profile
                    windows::core::PCWSTR(appcontainer_name.as_ptr()), // pszDisplayName: human-readable
                    windows::core::PCWSTR(appcontainer_name.as_ptr()), // pszDescription
                    None, // pCapabilities: none (== no capabilities)
                )
            } {
                Ok(sid) => {
                    return Ok(Self { _sa: Security::SECURITY_ATTRIBUTES::default(), _sd: vec![], acl: None, sid: Some(sid) });
                }
                Err(e) => {
                    if e.code() != ERROR_ALREADY_EXISTS.into() && e.code() != windows::Win32::Foundation::E_ACCESSDENIED.into() {
                        return Err(WindowsSandboxError::Setup(e));
                    }
                    // If it already exists, fall through and try another AppContainer name.
                    // Likewise, access denied can mean the AppContainer already exists but we don't have access to it,
                    // so try another name in that case as well.
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
            None => {
                return Err(WindowsSandboxError::setup_message(
                    "AppContainer already dropped",
                ));
            }
            Some(sid) => sid,
        };
        // This just really needs low-level access, as the windows crate doesn't provide it.
        // Windows will create the string in a buffer that this then needs to free.
        let mut buf_ptr: *mut u16 = std::ptr::null_mut();
        let result =
            unsafe { winapi::shared::sddl::ConvertSidToStringSidW(sid.0 as _, &mut buf_ptr) };

        if result == 0 {
            // No need to clean up
            Err(WindowsSandboxError::Sandbox(
                std::io::Error::last_os_error().into(),
            ))
        } else {
            // Create the OsString from the buffer, which is a null-terminated UTF-16 string,
            // and be sure to clean up the buffer.
            let mut nul_pos = 0;
            while unsafe { *buf_ptr.add(nul_pos) } != 0 {
                nul_pos += 1;
            }
            let slice_with_nul = unsafe { std::slice::from_raw_parts(buf_ptr, nul_pos + 1) };

            let os_string = OsString::from_wide(
                &slice_with_nul
                    .iter()
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
            Security::Isolation::GetAppContainerFolderPath(windows::core::PWSTR(sid.as_mut_ptr()))
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
        unsafe {
            if let Some(mut acl) = self.acl.take() {
                // ACL returned by SetEntriesInAclW is allocated with LocalAlloc; free with LocalFree.
                let _ = LocalFree(Some(HLOCAL(&mut acl)));
            }
        }        
    }
}
