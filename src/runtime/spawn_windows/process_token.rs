// SPDX-License-Identifier: MIT

//! Get the current process token.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security,
    System::Threading,
};

use crate::runtime::error::SandboxError;

use super::error::WindowsSandboxError;
use super::sid::{Sid, StoredSid};

pub struct ProcessToken {
    token: Option<HANDLE>,
}

impl ProcessToken {
    /// Create the process token for the current process.
    pub fn current_process() -> Result<Self, WindowsSandboxError> {
        let mut h_process_token = HANDLE::default();
        unsafe {
            Threading::OpenProcessToken(
                // derive restrictions from the current process.
                Threading::GetCurrentProcess(),
                Security::TOKEN_ALL_ACCESS,
                &mut h_process_token,
            )?
        };
        Ok(Self {
            token: Some(h_process_token),
        })
    }

    /// Create a restricted token from the process token.
    /// The restricted token has no privileges by using DISABLE_MAX_PRIVILEGE.
    /// However, with an AppContainer, this restricted token isn't workable.
    pub unsafe fn create_restricted_token(&self) -> Result<ProcessToken, WindowsSandboxError> {
        match self.token {
            None => Err(WindowsSandboxError::Sandbox(SandboxError::JailSetup(
                "already closed handle".to_string(),
            ))),
            Some(h) => {
                let disable_sids = build_powerful_sids_to_disable()?;
                let mut h_restricted = HANDLE::default();
                // Minimal: DISABLE_MAX_PRIVILEGE. You can also pass SIDs/privileges lists.
                unsafe {
                    Security::CreateRestrictedToken(
                        h,
                        Security::DISABLE_MAX_PRIVILEGE, // strips *all* privileges from the new token.
                        Some(disable_sids.sids.as_slice()), // disable high-power SIDs.
                        None, // no explicit privilege list (all are already stripped by DISABLE_MAX_PRIVILEGE)
                        None, // no restricting SIDs (we will move to tightening this later; misconfiguring it can break things easily)
                        &mut h_restricted,
                    )
                }
                .map(|_| ProcessToken {
                    token: Some(h_restricted),
                })
                .map_err(|e| WindowsSandboxError::setup(e))
            }
        }
    }

    pub fn none() -> ProcessToken {
        ProcessToken { token: None }
    }

    /// Returns the token's logon SID copied into Rust-owned bytes.
    pub fn current_logon_sid(&self) -> Result<StoredSid, WindowsSandboxError> {
        unsafe {
            let token = self.token.ok_or_else(|| {
                WindowsSandboxError::setup_message("process token handle is not available")
            })?;

            let mut needed: u32 = 0;
            let _ =
                Security::GetTokenInformation(token, Security::TokenLogonSid, None, 0, &mut needed);
            if needed == 0 {
                return Err(WindowsSandboxError::setup_message(
                    "TokenLogonSid query returned empty size",
                ));
            }

            let mut buf = vec![0u8; needed as usize];
            Security::GetTokenInformation(
                token,
                Security::TokenLogonSid,
                Some(buf.as_mut_ptr() as *mut _),
                needed,
                &mut needed,
            )?;

            let groups = &*(buf.as_ptr() as *const Security::TOKEN_GROUPS);
            if groups.GroupCount < 1 {
                return Err(WindowsSandboxError::setup_message(
                    "TokenLogonSid did not return a SID",
                ));
            }
            // Note that the .Sid value here exists within the structure, and requires
            // no additional Free* call.
            StoredSid::from_sid_copy(&groups.Groups[0].Sid)
        }
    }

    pub fn handle(&self) -> Option<HANDLE> {
        self.token
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        match self.token.take() {
            None => Ok(()),
            Some(h) => unsafe { CloseHandle(h) }.map_err(|e| WindowsSandboxError::Setup(e)),
        }
    }
}

impl Drop for ProcessToken {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Builds a list of "powerful/common" group SIDs to disable in a restricted token.
///
/// Notes:
/// - `CreateRestrictedToken(..., sidstodisable=Some(&[..]))` will mark matching groups in the new token
///   as *disabled/deny-only* so they no longer grant access via ACL checks.
/// - Disabling "admin-ish" groups is usually safe; disabling broad groups like "Users" or
///   "Authenticated Users" tends to break normal program execution and DLL loading.
///
/// The returned `SidBuffers` must be kept alive until after `CreateRestrictedToken` is called,
/// because `SID_AND_ATTRIBUTES.Sid` points into those buffers.
struct SidBuffers {
    _bufs: Vec<StoredSid>,
    pub sids: Vec<Security::SID_AND_ATTRIBUTES>,
}

fn build_powerful_sids_to_disable() -> Result<SidBuffers, WindowsSandboxError> {
    // Pick groups that commonly expand access on systems where the parent might be elevated
    // or where the user is in privileged local groups.
    //
    // These are "well-known SIDs" that can be constructed without querying the machine.
    let candidates: &[Security::WELL_KNOWN_SID_TYPE] = &[
        Security::WinBuiltinAdministratorsSid, // BUILTIN\Administrators
        Security::WinBuiltinPowerUsersSid,     // BUILTIN\Power Users (legacy but still present)
        Security::WinBuiltinBackupOperatorsSid, // BUILTIN\Backup Operators
        Security::WinBuiltinAccountOperatorsSid, // BUILTIN\Account Operators
        Security::WinBuiltinPrintOperatorsSid, // BUILTIN\Print Operators
        Security::WinBuiltinNetworkConfigurationOperatorsSid, // BUILTIN\Network Configuration Operators
        Security::WinBuiltinRemoteDesktopUsersSid, // BUILTIN\Remote Desktop Users (can matter in some environments)
    ];

    let mut bufs: Vec<StoredSid> = Vec::with_capacity(candidates.len());
    let mut sid_attrs: Vec<Security::SID_AND_ATTRIBUTES> = Vec::with_capacity(candidates.len());

    for &sid_type in candidates {
        let sid = StoredSid::new_well_known(sid_type)?;
        if let Some(s) = sid.sid() {
            // SID_AND_ATTRIBUTES:
            // - Sid: pointer to SID bytes (must remain valid during CreateRestrictedToken call)
            // - Attributes: for disable lists, typically 0 is fine. CreateRestrictedToken will
            //   disable the SID if it exists in the base token.
            sid_attrs.push(Security::SID_AND_ATTRIBUTES {
                Sid: s,
                Attributes: 0,
            });
            bufs.push(sid);
        }
    }

    Ok(SidBuffers {
        _bufs: bufs,
        sids: sid_attrs,
    })
}
