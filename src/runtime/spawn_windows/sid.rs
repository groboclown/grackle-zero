use std::{ffi::OsString, os::windows::ffi::OsStringExt};

// SPDX-License-Identifier: MIT
use windows::Win32::Security;

use crate::runtime::spawn_windows::error::WindowsSandboxError;

/// Store for a SID value.
pub trait Sid {
    fn sid(&self) -> Option<Security::PSID>;
}

/// Get the SID as an OS string.
pub fn into_osstr<T: Sid>(sid: &T) -> Result<Option<OsString>, WindowsSandboxError> {
    let sid = match sid.sid() {
        None => {
            return Ok(None);
        }
        Some(sid) => sid,
    };
    // This needs low-level access, as the windows crate doesn't provide it.
    // Windows will create the string in a buffer that this then needs to free.
    let mut buf_ptr: *mut u16 = std::ptr::null_mut();
    let result = unsafe { winapi::shared::sddl::ConvertSidToStringSidW(sid.0 as _, &mut buf_ptr) };

    if result == 0 {
        // No need to clean up
        Err(WindowsSandboxError::Sandbox(
            std::io::Error::last_os_error().into(),
        ))
    } else {
        // Create the OsString from the buffer, which is a null-terminated UTF-16 string,
        // and be sure to clean up the buffer.
        // As the buffer free is explicit, watch the '?' actions.

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

        Ok(Some(os_string))
    }
}

impl Sid for Box<dyn Sid> {
    fn sid(&self) -> Option<Security::PSID> {
        self.as_ref().sid()
    }
}

pub struct RawSid {
    sid: Option<Security::PSID>,
}

/// Wrap the raw PSID returned by another call in this drop safe structure.
/// Use this only for PSIDs whose lifetime contract requires `FreeSid`.
/// Examples include SIDs returned by AppContainer profile APIs such as
/// `CreateAppContainerProfile` and `DeriveAppContainerSidFromAppContainerName`.
///
/// Do not use this for PSIDs allocated by LocalAlloc-family APIs (for example
/// `ConvertStringSidToSidW`), because those must be released with `LocalFree`.
impl RawSid {
    pub fn new(sid: Security::PSID) -> Self {
        Self { sid: Some(sid) }
    }

    fn close(&mut self) {
        match self.sid.take() {
            Some(s) => {
                unsafe { Security::FreeSid(s) };
            }
            None => (),
        }
    }
}

impl Drop for RawSid {
    fn drop(&mut self) {
        self.close();
    }
}

impl Sid for RawSid {
    fn sid(&self) -> Option<Security::PSID> {
        self.sid
    }
}

/// Store a SID inside Rust-owned bytes.
pub struct StoredSid {
    stored: Option<Vec<u8>>,
    sid: Option<Security::PSID>,
}

impl StoredSid {
    /// Creates a SID backed by Rust-owned bytes.
    pub fn new_well_known(
        sid_type: Security::WELL_KNOWN_SID_TYPE,
    ) -> Result<Self, WindowsSandboxError> {
        let mut stored = vec![0u8; Security::SECURITY_MAX_SID_SIZE as usize];
        let mut sid_size: u32 = Security::SECURITY_MAX_SID_SIZE;
        let sid: Security::PSID = Security::PSID(stored.as_mut_ptr() as _);
        unsafe { Security::CreateWellKnownSid(sid_type, None, Some(sid), &mut sid_size)? };
        // Shrink to the actual SID size returned.
        stored.truncate(sid_size as usize);
        Ok(StoredSid {
            stored: Some(stored),
            sid: Some(sid),
        })
    }

    /// Creates a SID backed by Rust-owned bytes by copying from an existing PSID.
    /// The caller must perform the correct source SID cleanup.
    pub fn from_sid_copy(source: &Security::PSID) -> Result<Self, WindowsSandboxError> {
        unsafe {
            if source.0.is_null() {
                return Err(WindowsSandboxError::setup_message("source SID is null"));
            }
            let sid_len = Security::GetLengthSid(*source);
            if sid_len == 0 {
                return Err(WindowsSandboxError::setup_message(
                    "source SID length is zero",
                ));
            }
            let mut stored = vec![0u8; sid_len as usize];
            let sid: Security::PSID = Security::PSID(stored.as_mut_ptr() as _);
            Security::CopySid(sid_len, sid, *source)?;
            Ok(StoredSid {
                stored: Some(stored),
                sid: Some(sid),
            })
        }
    }

    fn close(&mut self) {
        self.sid.take();
        self.stored.take();
    }
}

impl Sid for StoredSid {
    fn sid(&self) -> Option<Security::PSID> {
        self.sid
    }
}

impl Drop for StoredSid {
    fn drop(&mut self) {
        self.close();
    }
}
