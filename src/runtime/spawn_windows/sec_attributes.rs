// SPDX-License-Identifier: MIT
use super::sid::Sid;
use std::rc::Rc;

use windows::Win32::Security;

use crate::runtime::spawn_windows::error::WindowsSandboxError;

/// Store for SECURITY_ATTRIBUTES.
pub struct SecurityAttributesWithAcl {
    _sids: Vec<Rc<Box<dyn Sid>>>,
    _mandatory_label_sid: Option<Rc<Box<dyn Sid>>>,
    sa: Box<Security::SECURITY_ATTRIBUTES>,
    _sd: Box<Security::SECURITY_DESCRIPTOR>,
    dacl: Option<Vec<u8>>,
    sacl: Option<Vec<u8>>,
}

pub struct AclEntry {
    pub sid: Rc<Box<dyn Sid>>,
    pub access_permissions: u32,
    pub ace_flags: Security::ACE_FLAGS,
}

impl SecurityAttributesWithAcl {
    pub fn default(sid: Rc<Box<dyn Sid>>) -> Self {
        Self {
            sa: Box::new(Security::SECURITY_ATTRIBUTES::default()),
            _sd: Box::new(Security::SECURITY_DESCRIPTOR::default()),
            dacl: None,
            sacl: None,
            _sids: vec![sid],
            _mandatory_label_sid: None,
        }
    }

    pub fn explicit_entries_with_mandatory_label(
        entries: Vec<AclEntry>,
        mandatory_label: Option<(Rc<Box<dyn Sid>>, u32)>,
    ) -> Result<Self, WindowsSandboxError> {
        unsafe {
            if entries.is_empty() {
                return Err(WindowsSandboxError::setup_message(
                    "At least one ACL SID is required",
                ));
            }

            // Resolve the target SID and calculate how many bytes are needed for one
            // ACCESS_ALLOWED_ACE in a DACL.
            //
            // dacl_len layout:
            //   1. ACL header (ACL)
            //   2. ACCESS_ALLOWED_ACE header (excluding SidStart placeholder)
            //   3. Full SID byte payload for the target SID
            //
            // This mirrors the C guidance that variable-sized ACE structures use
            // SidStart as a trailing placeholder.
            //
            // Ref:
            // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-getlengthsid
            // https://learn.microsoft.com/windows/win32/api/winnt/ns-winnt-access_allowed_ace
            // https://learn.microsoft.com/windows/win32/api/winnt/ns-winnt-acl
            let mut sid_ptrs: Vec<(Security::PSID, u32, Security::ACE_FLAGS)> =
                Vec::with_capacity(entries.len());
            let mut dacl_len = std::mem::size_of::<Security::ACL>();
            for entry in &entries {
                let sid_ptr = entry.sid.sid().ok_or_else(|| {
                    WindowsSandboxError::setup_message("Access SID is unavailable")
                })?;
                sid_ptrs.push((sid_ptr, entry.access_permissions, entry.ace_flags));
                let sid_len = Security::GetLengthSid(sid_ptr) as usize;
                dacl_len += (std::mem::size_of::<Security::ACCESS_ALLOWED_ACE>()
                    - std::mem::size_of::<u32>())
                    + sid_len;
            }
            let mut dacl_data = vec![0u8; dacl_len];
            let dacl_ptr = dacl_data.as_mut_ptr() as *mut Security::ACL;

            // Initialize DACL storage, then add one allow ACE for the target SID.
            // ACE_FLAGS(0) means this ACE applies directly and does not set
            // inheritance-only behavior.
            //
            // Ref:
            // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-initializeacl
            // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-addaccessallowedaceex
            Security::InitializeAcl(dacl_ptr, dacl_len as u32, Security::ACL_REVISION)?;
            for (sid_ptr, access_permissions, ace_flags) in sid_ptrs {
                Security::AddAccessAllowedAceEx(
                    dacl_ptr,
                    Security::ACL_REVISION,
                    ace_flags,
                    access_permissions,
                    sid_ptr,
                )?;
            }

            // Create and initialize a self-relative security descriptor container.
            // Ref: InitializeSecurityDescriptor
            // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-initializesecuritydescriptor
            let mut sd = Box::new(std::mem::zeroed::<Security::SECURITY_DESCRIPTOR>());
            let psec = Security::PSECURITY_DESCRIPTOR(sd.as_mut() as *mut _ as *mut _);

            Security::InitializeSecurityDescriptor(
                psec,
                windows::Win32::System::SystemServices::SECURITY_DESCRIPTOR_REVISION,
            )?;

            // Attach the DACL to the security descriptor.
            // Ref: SetSecurityDescriptorDacl
            // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-setsecuritydescriptordacl
            Security::SetSecurityDescriptorDacl(psec, true, Some(dacl_ptr), false)?;

            let mut sacl: Option<Vec<u8>> = None;
            let mut mandatory_label_sid_owned: Option<Rc<Box<dyn Sid>>> = None;
            if let Some((label_sid, mandatory_policy)) = mandatory_label {
                let label_sid_ptr = label_sid.sid().ok_or_else(|| {
                    WindowsSandboxError::setup_message("Mandatory label SID is unavailable")
                })?;
                // Compute the SACL byte size as:
                //   ACL header +
                //   one SYSTEM_MANDATORY_LABEL_ACE header (without SidStart placeholder) +
                //   full label SID length.
                // GetLengthSid returns the exact serialized SID byte count.
                // Ref:
                // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-getlengthsid
                // https://learn.microsoft.com/windows/win32/api/winnt/ns-winnt-system_mandatory_label_ace
                let label_sid_len = Security::GetLengthSid(label_sid_ptr) as usize;
                let sacl_len = std::mem::size_of::<Security::ACL>()
                    + (std::mem::size_of::<Security::SYSTEM_MANDATORY_LABEL_ACE>()
                        - std::mem::size_of::<u32>())
                    + label_sid_len;
                let mut sacl_data = vec![0u8; sacl_len];
                let sacl_ptr = sacl_data.as_mut_ptr() as *mut Security::ACL;
                // Initialize the SACL and add one mandatory-label ACE.
                // Ref:
                // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-initializeacl
                // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-addmandatoryace
                Security::InitializeAcl(sacl_ptr, sacl_len as u32, Security::ACL_REVISION)?;
                Security::AddMandatoryAce(
                    sacl_ptr,
                    Security::ACL_REVISION,
                    Security::ACE_FLAGS(0),
                    mandatory_policy,
                    label_sid_ptr,
                )?;
                // Attach the SACL to enforce mandatory integrity label policy.
                // Ref: SetSecurityDescriptorSacl
                // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-setsecuritydescriptorsacl
                Security::SetSecurityDescriptorSacl(psec, true, Some(sacl_ptr), false)?;

                mandatory_label_sid_owned = Some(label_sid);
                sacl = Some(sacl_data);
            }

            // Build SECURITY_ATTRIBUTES
            let sa = Box::new(Security::SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<Security::SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.as_mut() as *mut _ as *mut _,
                bInheritHandle: windows_core::BOOL(0),
            });

            Ok(SecurityAttributesWithAcl {
                sa: sa,
                _sd: sd,
                dacl: Some(dacl_data),
                sacl,
                _sids: entries.into_iter().map(|e| e.sid).collect::<Vec<_>>(),
                _mandatory_label_sid: mandatory_label_sid_owned,
            })
        }
    }

    pub fn attributes(&self) -> Option<*const Security::SECURITY_ATTRIBUTES> {
        match (&self.dacl, &self.sacl) {
            (None, None) => None,
            _ => Some(self.sa.as_ref() as *const _),
        }
    }
}
