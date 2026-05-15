// SPDX-License-Identifier: MIT

//! Wrapper for the AppContainer work.
//! Because much of windows requires explicit add/remove actions,
//! wrapping it in a single struct that implements Drop will make code maintenance easier.

use std::ffi::OsString;
use std::ffi::c_void;
use std::rc::Rc;
use std::time::Duration;
use windows::Win32::Foundation;
use windows::Win32::Security;
use windows::Win32::System::Threading;

use crate::restrictions;
use crate::runtime::spawn_windows::process_token::ProcessToken;

use super::conv::{as_c_str_w, hresult_eq, hresult_err_eq};
use super::error::WindowsSandboxError;
use super::sec_attributes::SecurityAttributesWithAcl;
use super::sid::{RawSid, Sid};

pub struct AppContainer {
    // name: String,
    // uid: String,
    sid: Option<Rc<Box<dyn Sid>>>,
    acl: Option<SecurityAttributesWithAcl>,
    drop_remove: Option<OsString>,
}

impl AppContainer {
    /// Create or retrieve the new app container.
    /// Note: the returned app container is persistent across executions, per user.
    /// This can construct heavyweight resources, and may take some time to run.
    pub fn new(restr: &restrictions::Restrictions) -> Result<Self, WindowsSandboxError> {
        let app_container_policy = match &restr.windows.app_container {
            restrictions::windows::AppContainerMode::Enabled(acp) => acp,
            restrictions::windows::AppContainerMode::Disabled => {
                return Ok(Self {
                    sid: None,
                    acl: None,
                    drop_remove: None,
                });
            }
        };
        let display_name = as_c_str_w(&OsString::from(&app_container_policy.name));

        // Notes on AppContainer Profiles and race conditions:
        // On AppContainer initial creation, the original call will wait to return until Windows
        // constructs all the elements of the AppContainer.  However, as this is expensive, it
        // can take a while to run.  This means that, while Windows runs through the processing
        // of creating the folders and other parts of the system, another process can also run this
        // same bit of code.  Windows may report that the AppContainer with the given name already exists
        // (ERROR_ALREADY_EXISTS) even though it hasn't completed the construction.  Sometimes, this may
        // show itself as an E_ACCESSDENIED error, depending on the specific state.  Because the AppContainer
        // construction is *OS* wide, not *process* wide, we can't try fun tricks like an in-memory mutex to
        // wait it out.  Therefore, this resorts to loops and sleeps.
        // However, we lessen the need for the extreme complex setup by creating a global, OS-wide lock.
        // This only runs at sandbox creation time, so the impact on performance should remain minimal.
        if app_container_policy.reuse_existing {
            let _init_lock = super::os_lock::OsLock::acquire(&app_container_policy.name)?;
            let os_name = OsString::from(&app_container_policy.name);
            match create_profile(&os_name, &display_name)? {
                CreateProfileResult::Created(sid) => {
                    return Ok(Self::from_created_profile(sid, os_name.clone(), true));
                }
                CreateProfileResult::PossiblyPending => {
                    for _ in 0..MAX_DISCOVERY_ATTEMPTS {
                        match find_existing_ready_profile(&os_name)? {
                            ExistingProfile::Ready(sid) => {
                                return Ok(Self::from_existing_profile(sid));
                            }
                            ExistingProfile::Missing => {
                                std::thread::sleep(DISCOVERY_WAIT);
                                continue;
                            }
                            ExistingProfile::NotReady => {
                                std::thread::sleep(DISCOVERY_WAIT);
                                continue;
                            }
                        }
                    }
                }
            }
            return Err(WindowsSandboxError::setup_message(
                "failed to create or discover shared AppContainer",
            ));
        }

        // Unique profile mode:
        // keep trying random names if there is a rare collision or transient deny.
        loop {
            let uid = super::rand::random_string_name(&app_container_policy.name)?;
            let os_name = OsString::from(uid);
            match create_profile(&os_name, &display_name)? {
                CreateProfileResult::Created(sid) => {
                    return Ok(Self::from_created_profile(sid, os_name, false));
                }
                CreateProfileResult::PossiblyPending => continue,
            }
        }
    }

    fn from_created_profile(sid: Security::PSID, os_name: OsString, reuse_existing: bool) -> Self {
        // Returned SID is released via FreeSid, so wrap as RawSid.
        let sid: Rc<Box<dyn Sid>> = Rc::new(Box::new(RawSid::new(sid)));
        Self {
            sid: Some(sid.clone()),
            acl: Some(SecurityAttributesWithAcl::default(sid)),
            drop_remove: if reuse_existing { None } else { Some(os_name) },
        }
    }

    fn from_existing_profile(sid: Rc<Box<dyn Sid>>) -> Self {
        Self {
            sid: Some(sid),
            acl: None,
            drop_remove: None,
        }
    }

    /// Get the AppContainer SID.
    pub fn sid(&self) -> Option<Rc<Box<dyn Sid>>> {
        match &self.sid {
            None => None,
            Some(s) => Some(s.clone()),
        }
    }

    /// Get the path to the AppContainer folder; similar to a User's folder.
    /// Returns an error if the AppContainer has already been dropped, or for general errors.
    pub fn folder_path(&self) -> Result<OsString, WindowsSandboxError> {
        match &self.sid {
            None => Err(WindowsSandboxError::setup_message(
                "AppContainer already dropped",
            )),
            Some(sid) => appcontainer_folder_path_for_sid(sid.as_ref()),
        }
    }

    pub unsafe fn create_child_process(
        &self,
        app: &[u16],
        cmdline: &mut [u16],
        has_allowed_handles: bool,
        creation_flags: Threading::PROCESS_CREATION_FLAGS,
        env: *const c_void,
        cwd: &[u16],
        startup_info: &Threading::STARTUPINFOW,
        process_info: &mut Threading::PROCESS_INFORMATION,
    ) -> Result<(), WindowsSandboxError> {
        if self.sid.is_some() {
            // AppContainer launch mode:
            // rely on PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES and CreateProcessW.
            unsafe {
                Threading::CreateProcessW(
                    windows::core::PCWSTR(app.as_ptr()),
                    Some(windows::core::PWSTR(cmdline.as_mut_ptr())),
                    None,
                    None,
                    has_allowed_handles,
                    creation_flags,
                    Some(env),
                    windows::core::PCWSTR(cwd.as_ptr()),
                    startup_info,
                    process_info,
                )
                .map_err(WindowsSandboxError::setup)?;
            }
            return Ok(());
        }

        // Non-AppContainer launch mode:
        // keep restricted-token behavior managed within AppContainer for now.
        let mut h_restricted = self.child_process_restricted_token()?;
        unsafe {
            Threading::CreateProcessAsUserW(
                h_restricted.handle(),
                windows::core::PCWSTR(app.as_ptr()),
                Some(windows::core::PWSTR(cmdline.as_mut_ptr())),
                None,
                None,
                has_allowed_handles,
                creation_flags,
                Some(env),
                windows::core::PCWSTR(cwd.as_ptr()),
                startup_info,
                process_info,
            )
            .map_err(WindowsSandboxError::setup)?;
        }
        h_restricted.close()?;
        Ok(())
    }

    fn child_process_restricted_token(&self) -> Result<ProcessToken, WindowsSandboxError> {
        match &self.sid {
            None => Ok(ProcessToken::none()),
            Some(_) => {
                let h_process_token = super::process_token::ProcessToken::current_process()?;
                unsafe { h_process_token.create_restricted_token() }
            }
        }
    }
}

// May want to make these constants configurable.
const MAX_DISCOVERY_ATTEMPTS: u32 = 100;
const DISCOVERY_WAIT: Duration = Duration::from_millis(20);

enum CreateProfileResult {
    Created(Security::PSID),
    PossiblyPending,
}

enum ExistingProfile {
    Ready(Rc<Box<dyn Sid>>),
    Missing,
    NotReady,
}

enum ExistingSid {
    Found(Rc<Box<dyn Sid>>),
    NotFound,
    NotReady,
}

fn create_profile(
    os_name: &OsString,
    display_name: &[u16],
) -> Result<CreateProfileResult, WindowsSandboxError> {
    // May want to add in WinAppContainerCapability capabilities.
    // See:
    // https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer
    match unsafe {
        Security::Isolation::CreateAppContainerProfile(
            windows::core::PCWSTR(as_c_str_w(os_name).as_ptr()),
            windows::core::PCWSTR(display_name.as_ptr()),
            windows::core::PCWSTR(display_name.as_ptr()),
            None,
        )
    } {
        Ok(sid) => Ok(CreateProfileResult::Created(sid)),
        Err(e) if hresult_err_eq(&e, Foundation::ERROR_ALREADY_EXISTS) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_err_eq(&e, Foundation::ERROR_KEY_DELETED) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_err_eq(&e, Foundation::ERROR_FILE_NOT_FOUND) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_err_eq(&e, Foundation::ERROR_PATH_NOT_FOUND) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_err_eq(&e, Foundation::ERROR_INVALID_HANDLE) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_eq(&e, Foundation::E_ACCESSDENIED) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) if hresult_eq(&e, Foundation::E_UNEXPECTED) => {
            Ok(CreateProfileResult::PossiblyPending)
        }
        Err(e) => Err(WindowsSandboxError::Setup(e)),
    }
}

fn find_existing_ready_profile(name: &OsString) -> Result<ExistingProfile, WindowsSandboxError> {
    let sid = match derive_existing_sid(name)? {
        ExistingSid::NotFound => return Ok(ExistingProfile::Missing),
        ExistingSid::NotReady => return Ok(ExistingProfile::NotReady),
        ExistingSid::Found(sid) => sid,
    };

    match appcontainer_folder_path_for_sid(sid.as_ref()) {
        Ok(path) => {
            if std::path::Path::new(&path).exists() {
                Ok(ExistingProfile::Ready(sid))
            } else {
                Ok(ExistingProfile::NotReady)
            }
        }
        Err(WindowsSandboxError::Setup(e)) => {
            // During concurrent first-time creation, SID discovery can succeed before
            // Windows has fully materialized the profile directory structure.
            if super::conv::hresult_err_eq(&e, Foundation::ERROR_FILE_NOT_FOUND)
                || super::conv::hresult_err_eq(&e, Foundation::ERROR_PATH_NOT_FOUND)
                || super::conv::hresult_err_eq(&e, Foundation::ERROR_KEY_DELETED)
            {
                Ok(ExistingProfile::NotReady)
            } else {
                Err(WindowsSandboxError::Setup(e))
            }
        }
        Err(e) => Err(e),
    }
}

/// Get the existing AppContainer SID.
fn derive_existing_sid(ac_name: &OsString) -> Result<ExistingSid, WindowsSandboxError> {
    match unsafe {
        Security::Isolation::DeriveAppContainerSidFromAppContainerName(windows::core::PCWSTR(
            as_c_str_w(ac_name).as_ptr(),
        ))
    } {
        Ok(sid) => Ok(ExistingSid::Found(Rc::new(Box::new(RawSid::new(sid))))),
        Err(e) if super::conv::hresult_err_eq(&e, Foundation::ERROR_NOT_FOUND) => {
            Ok(ExistingSid::NotFound)
        }
        Err(e) if super::conv::hresult_err_eq(&e, Foundation::ERROR_KEY_DELETED) => {
            Ok(ExistingSid::NotReady)
        }
        Err(e) if super::conv::hresult_eq(&e, Foundation::E_ACCESSDENIED) => {
            Ok(ExistingSid::NotReady)
        }
        Err(e) if super::conv::hresult_eq(&e, Foundation::E_UNEXPECTED) => {
            Ok(ExistingSid::NotReady)
        }
        Err(e) => Err(WindowsSandboxError::Setup(e)),
    }
}

fn appcontainer_folder_path_for_sid<T: Sid>(sid: &T) -> Result<OsString, WindowsSandboxError> {
    let sid_string = super::sid::into_osstr(sid)?
        .ok_or_else(|| WindowsSandboxError::setup_message("AppContainer SID is unavailable"))?;
    let mut sid_string = as_c_str_w(&sid_string);
    unsafe {
        Ok(super::conv::pwstr_as_osstring(
            Security::Isolation::GetAppContainerFolderPath(windows::core::PWSTR(
                sid_string.as_mut_ptr(),
            ))
            .map_err(WindowsSandboxError::setup)?,
        ))
    }
}

impl Drop for AppContainer {
    fn drop(&mut self) {
        self.acl.take();
        self.sid.take();
        match self.drop_remove.take() {
            None => (),
            Some(s) => {
                // Need to remove the Profile.
                let _ = unsafe {
                    Security::Isolation::DeleteAppContainerProfile(windows::core::PCWSTR(
                        as_c_str_w(&s).as_ptr(),
                    ))
                };
            }
        }
    }
}
