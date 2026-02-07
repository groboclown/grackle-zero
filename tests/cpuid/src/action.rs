// SPDX-License-Identifier: MIT

use super::debug::debug;

pub(crate) fn perform(action: String) {
    let mut has_ok = false;
    for c in action.as_str().chars() {
        let res = match c {
            // See the README.md for the character list.
            // * s - get the system id.
            's' => get_system_id(),

            // * o - get the OS name.
            'o' => get_os_name(),

            // * u - get the user name.
            'u' => get_username(),

            // * m - get the MAC address.
            'm' => get_mac_address(),

            // * c - get the CPU ID.
            'c' => get_cpu_id(),

            // * d - get the primary drive's serial number.
            'd' => get_drive_serial(),

            // * h - get the computer hostname.
            'h' => get_hostname(),

            // * q - get the number of CPU cores.
            'q' => get_core_count(),

            // * i - get the IP addresses.
            'i' => get_ip_address(),

            _ => Err("bad ID character".to_string()),
        };
        match res {
            Ok(val) => {
                debug(format!("[{}] = {}", c, val));
                has_ok = true;
            }
            Err(e) => {
                debug(format!("[{}] = ERROR {:?}", c, e));
            }
        }
    }
    if !has_ok {
        panic!("Encountered error fetching information");
    }
    // If *anything* is okay, allow this to pass.
    // That's because the test expects an error, and
    // we expect all of these to fail.
}

#[cfg(target_os = "linux")]
/// Get the linux system identifier.
fn get_system_id() -> Result<String, String> {
    std::fs::read_to_string("/etc/machine-id")
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("failed reading /etc/machine-id: {:?}", e))
}

#[cfg(target_os = "macos")]
/// Get the MacOS system identifier.
/// Note that general approaches to this require executing programs,
/// which is tested to be prevented elsewhere.
fn get_system_id() -> Result<String, String> {
    Err("executes programs".to_string())
}

#[cfg(target_os = "windows")]
mod u_windows {
    use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};
    use wmi::WMIConnection;

    pub fn read_local_machine_reg(key: &str, value: &str) -> Result<String, String> {
        use winreg::enums::{KEY_READ, KEY_WOW64_64KEY};

        let r_key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(key, KEY_READ | KEY_WOW64_64KEY)
            .map_err(|e| format!("failed reading {}: {:?}", key, e))?;

        let id = r_key.get_value(value)
            .map_err(|e| format!("failed reading {}/{}: {:?}", key, value, e))?;
        Ok(id)
    }

    pub fn wmi_query(query: &str) -> Result<Vec<String>, String> {
        let con = WMIConnection::new()
            .map_err(|e| format!("failed getting WMI connection: {:?}", e))?;
        con.raw_query(query)
            .map_err(|e| format!("failed running WMI query: {:?}", e))
    }
}

#[cfg(target_os = "windows")]
fn get_system_id() -> Result<String, String> {
    u_windows::read_local_machine_reg("SOFTWARE\\Microsoft\\Cryptography", "MachineGuid")
}

fn get_os_name() -> Result<String, String> {
    match sysinfo::System::long_os_version() {
        Some(m) => Ok(m),
        None => Err("could not find".to_string()),
    }
}

fn get_username() -> Result<String, String> {
    whoami::username().map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn get_mac_address() -> Result<String, String> {
    Err("not supported yet".to_string())
}

#[cfg(not(target_os = "windows"))]
fn get_mac_address() -> Result<String, String> {
    Err("not supported yet".to_string())
}

fn get_ip_address() -> Result<String, String> {
    Err("not supported yet".to_string())
}

fn get_cpu_id() -> Result<String, String> {
    let sys = sysinfo::System::new_all();
    let processors = sys.cpus();
    if processors.len() <= 0 {
        Err("no processors reported".to_string())
    } else {
        Ok(format!("{:?}", processors))
    }
}

#[cfg(target_os = "windows")]
fn get_drive_serial() -> Result<String, String> {
    match u_windows::wmi_query("SELECT SerialNumber FROM Win32_PhysicalMedia")?
        .get(0) {
        Some(v) => Ok(v.clone()),
        None => Err("no serial number found".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
fn get_drive_serial() -> Result<String, String> {
    Err("not supported yet".to_string())
}

fn get_hostname() -> Result<String, String> {
    match sysinfo::System::host_name() {
        Some(h) => Ok(h),
        None => Err("could not load".to_string()),
    }
}

fn get_core_count() -> Result<String, String> {
    match sysinfo::System::physical_core_count() {
        Some(m) => Ok(m.to_string()),
        None => Err("could not load".to_string()),
    }
}
