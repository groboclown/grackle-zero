//! Process/Thread Attribute List.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

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
        let attributes: Vec<Box<dyn ThreadAttribute>> = attributes.into_iter()
            .filter(|f| f.valid())
            .collect();
        if attributes.is_empty() {
            return Ok(Self{ _attributes: vec![], _attr_buf: vec![], attr_list: None });
        }
        unsafe {
            // ------------------------------------------------------------
            // Build the list.

            // Get the expected size.
            // This should return an error, which should indicate insufficient buffer size, which, yes,
            // we passed a 0 size to get the size.  It's weird semantics.
            let mut attr_size: usize = 0;
            match Threading::InitializeProcThreadAttributeList(
                None,                     // query buffer size
                attributes.len() as u32, // number of attributes to set
                Some(0),                          // must be 0
                &mut attr_size,                    // output required size in bytes
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
            let attr_list = Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr().cast::<_>());
            Threading::InitializeProcThreadAttributeList(
                Some(attr_list),          // allocated buffer
                attributes.len() as u32, // matches number of attributes to set
                Some(0),                          // must be 0
                &mut attr_size,                    // the computed size from the previous call
            )?;

            // Then load in the values.
            for attr in &attributes {
                Threading::UpdateProcThreadAttribute(
                    attr_list, // attribute list
                    0,                 // dwFlags must be 0
                    attr.attribute(),
                    attr.lp_value(),
                    attr.cb_size(),
                    None,      // not used; don't care about the previous value of this attribute.
                    None,         // not used; don't care about the size of the not-returned previous value.
                ).map_err(|e| WindowsSandboxError::setup(e))?;
            }

            Ok(Self { _attributes: attributes, _attr_buf: attr_buf, attr_list: Some(attr_list) })
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
