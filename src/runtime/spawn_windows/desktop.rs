// SPDX-License-Identifier: MIT

//! Creates an isolated alternate desktop / windows station for the spawned
//! process.
//! These isolate GUI state, prevent UI-driven attacks (input hooking,
//! clipboard, window messages), prevent the child from showing windows to
//! the interactive user or receiving input from other apps.
//!
//! In some circumstances, the execution context prohibits the generation of the
//! window station.  In this case, the sandbox just doesn't use it, and it means
//! the child will also be prohibited from using the UI.

use std::ffi::OsStr;
use std::rc::Rc;
use std::sync::Mutex;

use windows::Win32::{Foundation, Security, System::StationsAndDesktops, UI::WindowsAndMessaging};

use crate::restrictions;

use super::conv;
use super::error::WindowsSandboxError;
use super::sec_attributes;
use super::sid::Sid;

/// Captured from WinSta0 package ACEs: mask 0x20327.
/// Intentionally excludes station rights such as WINSTA_CREATEDESKTOP.
const STATION_MIN_ACL_MASK: u32 = (WindowsAndMessaging::WINSTA_ENUMDESKTOPS
    | WindowsAndMessaging::WINSTA_READATTRIBUTES
    | WindowsAndMessaging::WINSTA_ACCESSCLIPBOARD
    | WindowsAndMessaging::WINSTA_ACCESSGLOBALATOMS
    | WindowsAndMessaging::WINSTA_ENUMERATE
    | WindowsAndMessaging::WINSTA_READSCREEN) as u32
    | StationsAndDesktops::DESKTOP_READ_CONTROL.0;

/// Captured from WinSta0\\Default package ACEs: mask 0xF00FF.
/// Includes standard rights and desktop rights except DESKTOP_SWITCHDESKTOP.
const DESKTOP_MIN_ACL_MASK: u32 = StationsAndDesktops::DESKTOP_READOBJECTS.0
    | StationsAndDesktops::DESKTOP_CREATEWINDOW.0
    | StationsAndDesktops::DESKTOP_CREATEMENU.0
    | StationsAndDesktops::DESKTOP_HOOKCONTROL.0
    | StationsAndDesktops::DESKTOP_JOURNALRECORD.0
    | StationsAndDesktops::DESKTOP_JOURNALPLAYBACK.0
    | StationsAndDesktops::DESKTOP_ENUMERATE.0
    | StationsAndDesktops::DESKTOP_WRITEOBJECTS.0
    | StationsAndDesktops::DESKTOP_DELETE.0
    | StationsAndDesktops::DESKTOP_READ_CONTROL.0
    //| StationsAndDesktops::DESKTOP_WRITE_DAC.0
    //| StationsAndDesktops::DESKTOP_WRITE_OWNER.0
    ;

/// The create desktop desired access; ACL profile controls
/// what principals inside the sandbox can actually do on the object.
const DESKTOP_MIN_CREATE_MASK: u32 = StationsAndDesktops::DESKTOP_READOBJECTS.0
            | StationsAndDesktops::DESKTOP_CREATEWINDOW.0
            | StationsAndDesktops::DESKTOP_CREATEMENU.0
            | StationsAndDesktops::DESKTOP_HOOKCONTROL.0
            | StationsAndDesktops::DESKTOP_JOURNALRECORD.0
            | StationsAndDesktops::DESKTOP_JOURNALPLAYBACK.0
            | StationsAndDesktops::DESKTOP_ENUMERATE.0
            | StationsAndDesktops::DESKTOP_WRITEOBJECTS.0
            //| StationsAndDesktops::DESKTOP_SWITCHDESKTOP.0
            | StationsAndDesktops::DESKTOP_DELETE.0
            | StationsAndDesktops::DESKTOP_READ_CONTROL.0
            //| StationsAndDesktops::DESKTOP_WRITE_DAC.0
            //| StationsAndDesktops::DESKTOP_WRITE_OWNER.0
            //| StationsAndDesktops::DESKTOP_SYNCHRONIZE.0
            ;

pub struct UiIsolate {
    desktop: DesktopIsolate,
    // need to keep the station value around, to drop it only when the isolate is dropped.
    station: WindowStationIsolate,
    // Backing buffer for STARTUPINFOEXW.lpDesktop.
    // Must outlive CreateProcess* call.
    desktop_path: Option<Vec<u16>>,
}

static UI_PROCESS_LOCK: Mutex<bool> = Mutex::new(true);

impl UiIsolate {
    pub fn initialize(
        restr: &restrictions::Restrictions,
        app_sid: Option<Rc<Box<dyn Sid>>>,
    ) -> Result<Self, WindowsSandboxError> {
        let force_desktop_isolation = std::env::var_os("GRACKLE_FORCE_DESKTOP_ISOLATION")
            .map(|v| v == "1")
            .unwrap_or(false);
        let (name, app_sid) = match &restr.windows.app_container {
            restrictions::windows::AppContainerMode::Enabled(acp) => {
                if !acp.desktop_isolation && !force_desktop_isolation {
                    return Ok(Self {
                        desktop: DesktopIsolate {
                            name: None,
                            desktop: None,
                        },
                        station: WindowStationIsolate {
                            name: None,
                            station: None,
                        },
                        desktop_path: None,
                    });
                }
                (
                    randomized_desktop_name(&acp.name)?,
                    app_sid.ok_or_else(|| {
                        WindowsSandboxError::setup_message("missing AppContainer SID")
                    })?,
                )
            }
            restrictions::windows::AppContainerMode::Disabled => {
                if !force_desktop_isolation {
                    return Ok(Self {
                        desktop: DesktopIsolate {
                            name: None,
                            desktop: None,
                        },
                        station: WindowStationIsolate {
                            name: None,
                            station: None,
                        },
                        desktop_path: None,
                    });
                }
                (
                    randomized_desktop_name(&format!(
                        "gracklezero-desktop-{}",
                        std::process::id()
                    ))?,
                    current_logon_sid()?,
                )
            }
        };

        // This operation mutates process-wide UI state (window station switching),
        // so serialize the entire station/desktop construction under one lock.
        let _v = UI_PROCESS_LOCK
            .lock()
            .map_err(|e| WindowsSandboxError::from(e))?;

        // This requires a short time trick, which temporarily switches to the
        // new window station.
        let station = WindowStationIsolate::new(app_sid.clone())?;
        let mut old_station = None;

        // Set the station, create the desktop, then switch back to the old station.
        if let Some(station) = station.station {
            let os = unsafe { StationsAndDesktops::GetProcessWindowStation()? };
            if os.0 == std::ptr::null_mut() {
                // Not always fatal, but assume it is.
                return Err(WindowsSandboxError::setup_message(
                    "failed to get current window station",
                ));
            }
            unsafe { StationsAndDesktops::SetProcessWindowStation(station) }?;
            old_station = Some(os);
        }
        let desktop_res = DesktopIsolate::new(&name, app_sid);
        // Before returning the error, switch back to the old station, otherwise the process might be left
        // without a window station, which would cause all UI operations to fail.
        // Note that, if this itself fails, then the process is in a very bad state.
        // May want a specialized error just for this kind of case (UI in the parent process are now unable to work).
        if let Some(old_station) = old_station {
            unsafe { StationsAndDesktops::SetProcessWindowStation(old_station) }?;
        }
        let desktop = desktop_res?;

        let desktop = desktop;
        let desktop_path =
            if let (Some(winsta_name), Some(desktop_name)) = (&station.name, &desktop.name) {
                // STARTUPINFO.lpDesktop expects "winsta\\desktop" for non-default stations.
                let full_name = format!("{}\\{}", winsta_name, desktop_name);
                Some(conv::as_c_str_w(OsStr::new(&full_name)))
            } else {
                None
            };

        Ok(Self {
            desktop,
            station,
            desktop_path,
        })
    }

    pub fn lp_desktop(&self) -> windows::core::PWSTR {
        self.desktop_path
            .as_ref()
            .map(|p| windows::core::PWSTR(p.as_ptr() as *mut _))
            .unwrap_or(windows::core::PWSTR(std::ptr::null_mut()))
    }
}

impl Drop for UiIsolate {
    fn drop(&mut self) {
        // Ordering is important, so we explicitly define the drop.
        let _ = self.desktop_path.take();
        let _ = self.desktop.close();
        let _ = self.station.close();
    }
}

struct DesktopIsolate {
    name: Option<String>,
    desktop: Option<StationsAndDesktops::HDESK>,
}

impl DesktopIsolate {
    pub fn new(name: &str, acl_sid: Rc<Box<dyn Sid>>) -> Result<Self, WindowsSandboxError> {
        let s_name = name.to_string();
        let name = conv::as_c_str_w(OsStr::new(name));

        let entries = desktop_acl_entries(acl_sid, DESKTOP_MIN_ACL_MASK)?;
        let sec_attribs =
            sec_attributes::SecurityAttributesWithAcl::explicit_entries_with_mandatory_label(
                entries,
                Some((
                    Rc::new(Box::new(super::sid::StoredSid::new_well_known(
                        Security::WinUntrustedLabelSid,
                    )?)),
                    Security::TOKEN_MANDATORY_POLICY_NO_WRITE_UP.0,
                )),
            )?;

        match unsafe {
            // Ref: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createdesktopexw
            StationsAndDesktops::CreateDesktopExW(
                windows::core::PCWSTR(name.as_ptr()),    // desktop name
                windows::core::PCWSTR(std::ptr::null()), // device
                None,                                    // dev mode; must be NULL
                // Allowed control flags are only DF_ALLOWOTHERACCOUNTHOOK at the moment.
                StationsAndDesktops::DESKTOP_CONTROL_FLAGS(0),
                DESKTOP_MIN_CREATE_MASK,
                sec_attribs.attributes(),
                0, // 0 = use the default heap size
                None,
            )
        } {
            Ok(desktop) => Ok(Self {
                name: Some(s_name),
                desktop: Some(desktop),
            }),
            Err(e) => {
                if Self::is_desktop_creation_denied(&e) {
                    // This is expected in some contexts, so return None to indicate that the desktop could not be created.
                    // TODO use a proper logging mechanism here.
                    println!(
                        "Warning: failed to create desktop: {} (0x{:x})",
                        e.message(),
                        e.code().0,
                    );
                    Ok(Self {
                        name: None,
                        desktop: None,
                    })
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn is_desktop_creation_denied(err: &windows_result::Error) -> bool {
        let e_code = err.code().0;
        let r_code = (e_code & 0xFFFF) as u32;
        e_code == windows::Win32::Foundation::E_ACCESSDENIED.0
            || r_code == windows::Win32::Foundation::ERROR_NOT_ENOUGH_MEMORY.0
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        if let Some(desktop) = self.desktop.take() {
            unsafe { StationsAndDesktops::CloseDesktop(desktop) }.map_err(|e| e.into())
        } else {
            Ok(())
        }
    }
}

impl Drop for DesktopIsolate {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Windows Station
///
/// A window station is a securable object that contains a clipboard, a set of global atoms,
/// and a group of desktops.  A process can only have one window station at a time,
/// but it can switch between them.  By creating a new window station and switching to it,
/// we can isolate the child process from the interactive user and other apps, which
/// prevents some UI-driven attacks (input hooking, clipboard, window messages), prevents the
/// child from showing windows to the interactive user, and prevents the child from receiving
/// input from other apps.
///
/// This is a security-in-depth item.  It's not 100% necessary, as other mechanisms provide
/// similar protections.  Therefore, it's not an error scenario if this parent process
/// has access issues constructing a windows station.
struct WindowStationIsolate {
    name: Option<String>,
    station: Option<StationsAndDesktops::HWINSTA>,
}

impl WindowStationIsolate {
    pub fn new(acl_sid: Rc<Box<dyn Sid>>) -> Result<Self, WindowsSandboxError> {
        let acl_access = STATION_MIN_ACL_MASK;
        // Keep broad creator handle access for now; ACL profile controls sandbox principal rights.
        let create_access = WindowsAndMessaging::WINSTA_ALL_ACCESS as u32;
        let entries = station_acl_entries(acl_sid, acl_access)?;
        let sec_attribs =
            sec_attributes::SecurityAttributesWithAcl::explicit_entries_with_mandatory_label(
                entries,
                Some((
                    Rc::new(Box::new(super::sid::StoredSid::new_well_known(
                        Security::WinUntrustedLabelSid,
                    )?)),
                    Security::TOKEN_MANDATORY_POLICY_NO_WRITE_UP.0,
                )),
            )?;

        // Windows Stations must use a NULL name; only admins may specify a name.
        match unsafe {
            StationsAndDesktops::CreateWindowStationW(
                windows::core::PCWSTR(std::ptr::null_mut()),
                0,
                create_access,
                sec_attribs.attributes(),
            )
        } {
            Err(e) => Err(e.into()),
            Ok(station) => {
                if station.0 == std::ptr::null_mut() {
                    Err(WindowsSandboxError::setup_message(
                        "failed to create window station",
                    ))
                } else {
                    Self::new_from_handle(station)
                }
            }
        }
    }

    fn new_from_handle(handle: StationsAndDesktops::HWINSTA) -> Result<Self, WindowsSandboxError> {
        let mut needed: u32 = 0;
        let h = Foundation::HANDLE(handle.0);

        // First call asks for required size in bytes.
        let _ = unsafe {
            StationsAndDesktops::GetUserObjectInformationW(
                h,
                StationsAndDesktops::UOI_NAME,
                None,
                0,
                Some(&mut needed),
            )
        };

        if needed == 0 {
            let err = unsafe { Foundation::GetLastError() };
            let err: windows::core::Error = err.into();
            return Err(WindowsSandboxError::Setup(err.into()));
        }

        // needed is bytes, buffer is UTF-16
        let wchar_count = (needed as usize + 1) / 2;
        let mut buf = vec![0u16; wchar_count];

        unsafe {
            StationsAndDesktops::GetUserObjectInformationW(
                h,
                StationsAndDesktops::UOI_NAME,
                Some(buf.as_mut_ptr() as *mut _),
                needed,
                Some(&mut needed),
            )
        }?;

        let name = conv::c_str_w_as_str(&buf);
        Ok(Self {
            name: Some(name),
            station: Some(handle),
        })
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        if let Some(station) = self.station.take() {
            unsafe { StationsAndDesktops::CloseWindowStation(station) }.map_err(|e| e.into())
        } else {
            Ok(())
        }
    }
}

impl Drop for WindowStationIsolate {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn randomized_desktop_name(prefix: &str) -> Result<String, WindowsSandboxError> {
    super::rand::random_str_name(prefix)
}

fn acl_principals(app_sid: Rc<Box<dyn Sid>>) -> Result<Vec<Rc<Box<dyn Sid>>>, WindowsSandboxError> {
    // Principal set for UI objects:
    // 1) exact AppContainer SID
    // 2) All Application Packages
    // 3) All Restricted Application Packages
    // 4) local logon SID group
    Ok(vec![
        app_sid,
        Rc::new(Box::new(super::sid::StoredSid::new_well_known(
            Security::WinBuiltinAnyPackageSid,
        )?)),
        all_restricted_app_packages_sid()?,
        current_logon_sid()?,
    ])
}

fn all_restricted_app_packages_sid() -> Result<Rc<Box<dyn Sid>>, WindowsSandboxError> {
    // Construct S-1-15-2-2 (ALL RESTRICTED APPLICATION PACKAGES) explicitly.
    // SID layout:
    //   IdentifierAuthority = SECURITY_APP_PACKAGE_AUTHORITY (15)
    //   SubAuthority[0] = SECURITY_BUILTIN_PACKAGE_ANY_RESTRICTED_PACKAGE (2)
    //   SubAuthority[1] = SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE (2)
    //
    // Ref:
    // https://learn.microsoft.com/windows/win32/secauthz/well-known-sids
    // https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-allocateandinitializesid
    let mut sid = Security::PSID::default();
    unsafe {
        Security::AllocateAndInitializeSid(
            &Security::SECURITY_APP_PACKAGE_AUTHORITY,
            2,
            2,
            2,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        )?;
    }
    Ok(Rc::new(Box::new(super::sid::RawSid::new(sid))))
}

fn desktop_acl_entries(
    app_sid: Rc<Box<dyn Sid>>,
    desktop_access: u32,
) -> Result<Vec<sec_attributes::AclEntry>, WindowsSandboxError> {
    Ok(acl_principals(app_sid)?
        .into_iter()
        .map(|sid| sec_attributes::AclEntry {
            sid,
            // For now, grant the same explicit desktop access to each principal.
            // We will minimize this after we identify the exact required subset.
            access_permissions: desktop_access,
            ace_flags: Security::ACE_FLAGS(0),
        })
        .collect::<Vec<_>>())
}

fn station_acl_entries(
    app_sid: Rc<Box<dyn Sid>>,
    station_access: u32,
) -> Result<Vec<sec_attributes::AclEntry>, WindowsSandboxError> {
    Ok(acl_principals(app_sid)?
        .into_iter()
        .map(|sid| sec_attributes::AclEntry {
            sid,
            // For now, grant the same explicit station access to each principal.
            // We will minimize this after we identify the exact required subset.
            access_permissions: station_access,
            ace_flags: Security::ACE_FLAGS(0),
        })
        .collect::<Vec<_>>())
}

fn current_logon_sid() -> Result<Rc<Box<dyn Sid>>, WindowsSandboxError> {
    let token = super::process_token::ProcessToken::current_process()?;
    let sid = token.current_logon_sid()?;
    Ok(Rc::new(Box::new(sid)))
}
