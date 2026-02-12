// SPDX-License-Identifier: MIT

//! Get the current process token.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE}, Security, System::Threading
};

use crate::runtime::error::SandboxError;

use super::error::WindowsSandboxError;

pub struct ProcessToken {
    token: Option<HANDLE>,
}

impl ProcessToken {
    /// Create the process token for the current process.
    pub fn current_process() -> Result<Self, WindowsSandboxError> {
        let mut h_process_token = HANDLE::default();
        unsafe { Threading::OpenProcessToken( // derive restrictions from the current process.
            Threading::GetCurrentProcess(),
            Security::TOKEN_ALL_ACCESS,
            &mut h_process_token,
        )? };
        Ok(Self {
            token: Some(h_process_token),
        })
    }

    /// Create a restricted token from the process token.
    /// The restricted token has no privileges by using DISABLE_MAX_PRIVILEGE.
    /// However, with an AppContainer, this restricted token isn't workable.
    pub unsafe fn create_restricted_token(&self) -> Result<ProcessToken, WindowsSandboxError> {
        match self.token {
            None => Err(WindowsSandboxError::Sandbox(SandboxError::JailSetup("already closed handle".to_string()))),
            Some(h) => {
                let disable_sids = build_powerful_sids_to_disable()?;
                let mut h_restricted = HANDLE::default();
                // Minimal: DISABLE_MAX_PRIVILEGE. You can also pass SIDs/privileges lists.
                unsafe { Security::CreateRestrictedToken(
                    h,
                    Security::DISABLE_MAX_PRIVILEGE, // strips *all* privileges from the new token.
                    Some(disable_sids.sids.as_slice()),  // disable high-power SIDs.
                    None,               // no explicit privilege list (all are already stripped by DISABLE_MAX_PRIVILEGE)
                    None,                   // no restricting SIDs (we will move to tightening this later; misconfiguring it can break things easily)
                    &mut h_restricted
                ) }
                    .map(|_| ProcessToken{ token: Some(h_restricted) })
                    .map_err(|e| WindowsSandboxError::setup(e))
            }
        }
    }

    pub fn handle(&self) -> Option<HANDLE> {
        self.token
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        match self.token.take() {
            None => Ok(()),
            Some(h) => {
                unsafe { CloseHandle(h) }.map_err(|e| WindowsSandboxError::Setup(e))
            }
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
    _bufs: Vec<Vec<u8>>,
    pub sids: Vec<Security::SID_AND_ATTRIBUTES>,
}

fn build_powerful_sids_to_disable() -> Result<SidBuffers, WindowsSandboxError> {
    // Pick groups that commonly expand access on systems where the parent might be elevated
    // or where the user is in privileged local groups.
    //
    // These are "well-known SIDs" that can be constructed without querying the machine.
    let candidates: &[Security::WELL_KNOWN_SID_TYPE] = &[
        Security::WinBuiltinAdministratorsSid,            // BUILTIN\Administrators
        Security::WinBuiltinPowerUsersSid,                // BUILTIN\Power Users (legacy but still present)
        Security::WinBuiltinBackupOperatorsSid,           // BUILTIN\Backup Operators
        Security::WinBuiltinAccountOperatorsSid,          // BUILTIN\Account Operators
        Security::WinBuiltinPrintOperatorsSid,            // BUILTIN\Print Operators
        Security::WinBuiltinNetworkConfigurationOperatorsSid, // BUILTIN\Network Configuration Operators
        Security::WinBuiltinRemoteDesktopUsersSid,        // BUILTIN\Remote Desktop Users (can matter in some environments)
    ];

    let mut bufs: Vec<Vec<u8>> = Vec::with_capacity(candidates.len());
    let mut sid_attrs: Vec<Security::SID_AND_ATTRIBUTES> = Vec::with_capacity(candidates.len());

    for &sid_type in candidates {
        // Allocate a buffer large enough for a SID. SECURITY_MAX_SID_SIZE is an upper bound.
        let mut sid_buf = vec![0u8; Security::SECURITY_MAX_SID_SIZE as usize];
        let mut sid_size: u32 = sid_buf.len() as u32;

        unsafe {
            // CreateWellKnownSid(
            //   WellKnownSidType: which SID to create (e.g., Administrators)
            //   DomainSid: used only for domain-relative SIDs; for BUILTIN groups, pass NULL
            //   pSid: output SID bytes
            //   cbSid: in/out size of buffer in bytes
            Security::CreateWellKnownSid(
                sid_type,
                None,
                Some(Security::PSID(sid_buf.as_mut_ptr() as _)),
                &mut sid_size,
            )?;

            // Shrink to the actual SID size returned.
            sid_buf.truncate(sid_size as usize);

            // SID_AND_ATTRIBUTES:
            // - Sid: pointer to SID bytes (must remain valid during CreateRestrictedToken call)
            // - Attributes: for disable lists, typically 0 is fine. CreateRestrictedToken will
            //   disable the SID if it exists in the base token.
            sid_attrs.push(Security::SID_AND_ATTRIBUTES {
                Sid: Security::PSID(sid_buf.as_mut_ptr() as _),
                Attributes: 0,
            });
        }

        bufs.push(sid_buf);
    }

    Ok(SidBuffers { _bufs: bufs, sids: sid_attrs })
}
