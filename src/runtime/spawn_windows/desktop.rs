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

use windows::Win32::{
    System::StationsAndDesktops,
    UI::WindowsAndMessaging,
};

use super::error::WindowsSandboxError;
use super::conv;

pub struct UiIsolate {
    desktop: DesktopIsolate,
    // need to keep the station value around, to drop it only when the isolate is dropped.
    station: WindowStationIsolate,
}

impl UiIsolate {
    pub fn initialize(name: &str) -> Result<Self, WindowsSandboxError> {
        // This requires a short time trick, which temporarily switches to the
        // new window station.
        let station = WindowStationIsolate::new(name)?;
        let mut old_station = None;
        // Set the station, create the desktop, then switch back to the old station.
        if let Some(station) = station.station {
            let os = unsafe { StationsAndDesktops::GetProcessWindowStation()? };
            if os.0 == std::ptr::null_mut() {
                // Not always fatal, but assume it is.
                return Err(WindowsSandboxError::setup_message("failed to get current window station"));
            }
            unsafe { StationsAndDesktops::SetProcessWindowStation(station) }?;
            old_station = Some(os);
        }
        let desktop_res = DesktopIsolate::new(name);
        // Before returning the error, switch back to the old station, otherwise the process might be left
        // without a window station, which would cause all UI operations to fail.
        // Note that, if this itself fails, then the process is in a very bad state.
        // May want a specialized error just for this kind of case (UI in the parent process are now unable to work).
        if let Some(old_station) = old_station {
            unsafe { StationsAndDesktops::SetProcessWindowStation(old_station) }?;
        }

        Ok(Self {
            desktop: desktop_res?,
            station: station,
        })
    }

    pub fn lp_desktop(&self) -> windows::core::PWSTR {
        if let (Some(winsta_name), Some(desktop_name)) = (&self.station.name, &self.desktop.name) {
            // This is the expected case, where we successfully created the station and desktop, so we can return the actual name.
            // The format for the desktop is "winsta_name\\desktop_name".
            let full_name = format!("{}\\{}", winsta_name, desktop_name);
            let full_name = conv::as_c_str_w(OsStr::new(&full_name));
            windows::core::PWSTR(full_name.as_ptr() as *mut _)
        } else {
            windows::core::PWSTR(std::ptr::null_mut())
        }
    }
}

impl Drop for UiIsolate {
    fn drop(&mut self) {
        // Ordering is important, so we explicitly define the drop.
        let _ = self.desktop.close();
        let _ = self.station.close();
    }
}

struct DesktopIsolate {
    name: Option<String>,
    desktop: Option<StationsAndDesktops::HDESK>,
}

impl DesktopIsolate {
    pub fn new(name: &str) -> Result<Self, WindowsSandboxError> {
        let s_name = name.to_string();
        let name = conv::as_c_str_w(OsStr::new(name));
        match unsafe { StationsAndDesktops::CreateDesktopExW(
            windows::core::PCWSTR(name.as_ptr()),
            windows::core::PCWSTR(std::ptr::null()),
            None,
            StationsAndDesktops::DESKTOP_CONTROL_FLAGS(0),
            // Set permissions for the UI creation.
            // Here are minimal rights.
            0,
            None,
            0, // use the default heap size
            None,
        ) } {
            Ok(desktop) => Ok(Self { name: Some(s_name), desktop: Some(desktop), }),
            Err(e) => {
                if Self::is_desktop_access_denied(&e) {
                    // This is expected in some contexts, so return None to indicate that the desktop could not be created.
                    // TODO use a proper logging mechanism here.
                    println!(
                        "Warning: failed to create desktop, error: {:?}.  This is expected in some contexts, but it means the child process won't be able to use the UI.",
                        e,
                    );
                    Ok(Self { name: None, desktop: None })
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn is_desktop_access_denied(err: &windows_result::Error) -> bool {
        let e_code = err.code().0;
        let r_code = (e_code & 0xFFFF) as u32;
        e_code == windows::Win32::Foundation::E_ACCESSDENIED.0
        || r_code == windows::Win32::Foundation::ERROR_NOT_ENOUGH_MEMORY.0
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        if let Some(desktop) = self.desktop.take() {
            unsafe{ StationsAndDesktops::CloseDesktop(desktop) }
               .map_err(|e| e.into())
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
    pub fn new(name: &str) -> Result<Self, WindowsSandboxError> {
        // FIXME create a unique name if the station already exists, to avoid conflicts with other instances of the sandbox.
        let mut idx = 0;
        loop {
            let uid = format!("{}{}", name, idx);
            idx += 1;
            let s_name = uid.to_string();
            let i_name = conv::as_c_str_w(OsStr::new(&uid));
            match unsafe {
                StationsAndDesktops::CreateWindowStationW(
                    windows::core::PCWSTR(i_name.as_ptr()),
                    0,
                    WindowsAndMessaging::WINSTA_ALL_ACCESS as u32,
                    None,
                )
            } {
                Err(e) => {
                    if !Self::is_station_access_denied(&e) {
                        return Err(e.into());
                    }
                }
                Ok(station) => {
                    if station.0 == std::ptr::null_mut() {
                        return Err(WindowsSandboxError::setup_message("failed to create window station"));
                    } else {
                        return Ok(Self { name: Some(s_name), station: Some(station) });
                    }
                }
            }
        }
    }

    fn is_station_access_denied(err: &windows_result::Error) -> bool {
        let code = err.code().0;
        code == windows::Win32::Foundation::E_ACCESSDENIED.0
    }

    pub fn close(&mut self) -> Result<(), WindowsSandboxError> {
        if let Some(station) = self.station.take() {
            unsafe { StationsAndDesktops::CloseWindowStation(station) }
                .map_err(|e| e.into())
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
