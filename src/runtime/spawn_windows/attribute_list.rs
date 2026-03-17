// SPDX-License-Identifier: MIT

//! Process/Thread Attribute List.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use winapi::shared::minwindef::DWORD;
use windows::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, GetLastError, HANDLE},
    Security,
    System::Threading,
};

use crate::runtime::spawn_windows::error::WindowsSandboxError;

pub trait ThreadAttribute {
    fn valid(&self) -> bool;
    fn lp_value(&self) -> Option<*const core::ffi::c_void>;
    fn attribute(&self) -> usize;
    fn cb_size(&self) -> usize;
}

pub type ThreadAttributeHandles = Vec<HANDLE>;

impl ThreadAttribute for ThreadAttributeHandles {
    fn valid(&self) -> bool {
        !self.is_empty()
    }

    fn lp_value(&self) -> Option<*const core::ffi::c_void> {
        if self.is_empty() {
            None
        } else {
            Some(self.as_ptr() as *const core::ffi::c_void)
        }
    }

    fn attribute(&self) -> usize {
        Threading::PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize
    }

    fn cb_size(&self) -> usize {
        self.len() * std::mem::size_of::<HANDLE>()
    }
}

pub type ThreadAttributeSecurityCapabilities = Security::SECURITY_CAPABILITIES;

impl ThreadAttribute for ThreadAttributeSecurityCapabilities {
    fn valid(&self) -> bool {
        true
    }

    fn lp_value(&self) -> Option<*const core::ffi::c_void> {
        Some((self as *const Security::SECURITY_CAPABILITIES).cast())
    }
    fn attribute(&self) -> usize {
        Threading::PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize
    }
    fn cb_size(&self) -> usize {
        std::mem::size_of::<Security::SECURITY_CAPABILITIES>()
    }
}

pub type ThreadAttributeChildProcessRestriction = DWORD;

impl ThreadAttribute for ThreadAttributeChildProcessRestriction {
    fn valid(&self) -> bool {
        true
    }

    fn lp_value(&self) -> Option<*const core::ffi::c_void> {
        Some((self as *const DWORD).cast())
    }
    fn attribute(&self) -> usize {
        Threading::PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY as usize
    }
    fn cb_size(&self) -> usize {
        std::mem::size_of::<DWORD>()
    }
}

/// The process being created is not allowed to create child processes.
/// Only effective within AppContainer sandboxes.
pub const NO_CHILD_PROCESS_RESTRICTION: ThreadAttributeChildProcessRestriction = 1;

// See https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute

pub type ThreadAttributeMitigationPolicyFlag = usize;  // dword64 on 64 bit systems, dword in 32 bit systems.

pub mod policy_flags {
    use super::ThreadAttributeMitigationPolicyFlag;
    pub const PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE: ThreadAttributeMitigationPolicyFlag = 0x00000001;
    pub const PROCESS_CREATION_MITIGATION_POLICY_DEP_ATL_THUNK_ENABLE: ThreadAttributeMitigationPolicyFlag = 0x00000002;
    pub const PROCESS_CREATION_MITIGATION_POLICY_SEHOP_ENABLE: ThreadAttributeMitigationPolicyFlag = 0x00000004;
    pub const PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON_REQ_RELOCS: ThreadAttributeMitigationPolicyFlag =  0x00000003 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 12;
    pub const PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 12;
    pub const PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 16;
    pub const PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 16;
    pub const PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 20;
    pub const PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 20;
    pub const PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 24;
    pub const PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 24;
    pub const PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 28;
    pub const PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 28;
    pub const PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 32; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 32; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 40;
    pub const PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 40; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 40;
    pub const PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 40; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_EXPORT_SUPPRESSION: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 40; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 8;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 8; 
    pub const PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_RESERVED: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 8; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 36;
    pub const PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 36; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 36; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 36; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON_ALLOW_OPT_OUT: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 36; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 44;
    pub const PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 44; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 44;
    pub const PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 44; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALLOW_STORE: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 44; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 48;
    pub const PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 48; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 48; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 48;   
    pub const PROCESS_CREATION_MITIGATION_POLICY_AUDIT_NONSYSTEM_FONTS: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 48; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 52;
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 52;
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 52;
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 52;
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_RESERVED: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 52;
    // pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 56; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 56; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 56; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 56; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_RESERVED: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 56; 
    // pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_MASK: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 60; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_DEFER: ThreadAttributeMitigationPolicyFlag = 0x00000000 << 60; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 60; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 60; 
    pub const PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_RESERVED: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 60; 

    // The following value is available only in Windows 10, version 1709 or later and only with the January 2018 Windows security updates and any applicable firmware updates from the OEM device manufacturer.
    pub const PROCESS_CREATION_MITIGATION_POLICY2_RESTRICT_INDIRECT_BRANCH_PREDICTION_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 16;

    // The following value is available only in Windows 10, version 1809 or later. 
    pub const PROCESS_CREATION_MITIGATION_POLICY2_SPECULATIVE_STORE_BYPASS_DISABLE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 24;

    // The following values are available only in Windows 10, version 2004 or later.
    pub const PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 28;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 28;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_STRICT_MODE: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 28;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 32;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 32;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_RELAXED_MODE: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 32;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 36;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 36;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_NON_EHCONT: ThreadAttributeMitigationPolicyFlag = 0x00000003 << 36;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 48;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 48;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_ON: ThreadAttributeMitigationPolicyFlag = 0x00000001 << 56;
    pub const PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_OFF: ThreadAttributeMitigationPolicyFlag = 0x00000002 << 56;
}

pub struct ThreadAttributeMitigationPolicy {
    flags: Vec<ThreadAttributeMitigationPolicyFlag>,
}

impl ThreadAttributeMitigationPolicy {
    pub fn vec(flags: Vec<ThreadAttributeMitigationPolicyFlag>) -> Self {
        Self { flags }
    }

    pub fn one(flag: ThreadAttributeMitigationPolicyFlag) -> Self {
        Self { flags: vec![flag] }
    }

    pub fn slice(flags: &[ThreadAttributeMitigationPolicyFlag]) -> Self {
        Self { flags: flags.to_vec() }
    }
}

impl From<ThreadAttributeMitigationPolicy> for ThreadAttributeMitigationPolicyFlag {
    fn from(value: ThreadAttributeMitigationPolicy) -> Self {
        let mut result: ThreadAttributeMitigationPolicyFlag = 0;
        for flag in value.flags {
            result |= flag;
        }
        result
    }
}

impl ThreadAttribute for ThreadAttributeMitigationPolicyFlag {
    fn valid(&self) -> bool {
        true
    }

    fn lp_value(&self) -> Option<*const core::ffi::c_void> {
        Some((self as *const _ as *const core::ffi::c_void).cast())
    }
    fn attribute(&self) -> usize {
        Threading::PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY as usize
    }
    fn cb_size(&self) -> usize {
        std::mem::size_of::<ThreadAttributeMitigationPolicyFlag>()
    }
}




pub struct ThreadAttributeList {
    // This holds on to the ThreadAttribute values to ensure the pointers don't change on it.
    // This may not be necessary.  The UpdateProcThreadAttribute call may copy the memory
    // from user space into kernel space, in which case the memory can go out of scope.
    _attributes: Vec<Box<dyn ThreadAttribute>>,
    // attr_buf contains the actual buffer that the attribute list uses.  It must be
    // maintained in the structure to ensure the memory is not freed while the attribute list is in use.
    _attr_buf: Vec<u8>,
    attr_list: Option<Threading::LPPROC_THREAD_ATTRIBUTE_LIST>,
}

impl ThreadAttributeList {
    pub fn new(attributes: Vec<Box<dyn ThreadAttribute>>) -> Result<Self, WindowsSandboxError> {
        let attributes: Vec<Box<dyn ThreadAttribute>> =
            attributes.into_iter().filter(|f| f.valid()).collect();
        if attributes.is_empty() {
            return Ok(Self {
                _attributes: vec![],
                _attr_buf: vec![],
                attr_list: None,
            });
        }
        unsafe {
            // ------------------------------------------------------------
            // Build the list.

            // Get the expected size.
            // This should return an error, which should indicate insufficient buffer size, which, yes,
            // we passed a 0 size to get the size.  It's weird semantics.
            let mut attr_size: usize = 0;
            match Threading::InitializeProcThreadAttributeList(
                None,                    // query buffer size
                attributes.len() as u32, // number of attributes to set
                Some(0),                 // must be 0
                &mut attr_size,          // output required size in bytes
            ) {
                Ok(_) => (), // Unexpected, but we'll allow it.
                Err(e) => {
                    let last_err = GetLastError();
                    if last_err != ERROR_INSUFFICIENT_BUFFER {
                        // It's not the expected error, so it's a real error.
                        return Err(WindowsSandboxError::setup(e));
                    }
                }
            };

            // Then initialize it.
            let mut attr_buf = vec![0u8; attr_size];
            let attr_list =
                Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr().cast::<_>());
            Threading::InitializeProcThreadAttributeList(
                Some(attr_list),         // allocated buffer
                attributes.len() as u32, // matches number of attributes to set
                Some(0),                 // must be 0
                &mut attr_size,          // the computed size from the previous call
            )?;

            // Then load in the values.
            for attr in &attributes {
                Threading::UpdateProcThreadAttribute(
                    attr_list, // attribute list
                    0,         // dwFlags must be 0
                    attr.attribute(),
                    attr.lp_value(),
                    attr.cb_size(),
                    None, // not used; don't care about the previous value of this attribute.
                    None, // not used; don't care about the size of the not-returned previous value.
                )
                .map_err(|e| WindowsSandboxError::setup(e))?;
            }

            Ok(Self {
                _attributes: attributes,
                _attr_buf: attr_buf,
                attr_list: Some(attr_list),
            })
        }
    }

    pub fn list(&self) -> Threading::LPPROC_THREAD_ATTRIBUTE_LIST {
        self.attr_list.expect("already dropped the attribute list")
    }
}

impl Drop for ThreadAttributeList {
    fn drop(&mut self) {
        match self.attr_list.take() {
            None => (),
            Some(list) => {
                let _ = unsafe { Threading::DeleteProcThreadAttributeList(list) };
            }
        }
    }
}
