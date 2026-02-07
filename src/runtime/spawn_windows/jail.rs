// SPDX-License-Identifier: MIT

use std::{ffi, mem, os::windows::ffi::OsStrExt};
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, GetLastError, HANDLE},
    Security,
    System::{JobObjects, Threading},
};


#[derive(Clone)]
pub struct ProcessInfo {
    pub process: HANDLE,
    pub thread: HANDLE,
    pub job: HANDLE,
}

/// Spawn the executable in a restricted mode.
/// Make sure to pass the handles through the `prepare_inheritance_allowlist` function.
/// Construct the cmdline argument with the `launch_quote::quote_arguments` function.
/// Construct the env argument with the `launch_quote::encode_env_strings` function.
/// 
/// TODO look at switching the arguments to instead use OsStr, because windows infamously allows
/// non-unicode valid characters as filenames.
pub fn launch_restricted<'a, 'b, 'c, 'd>(
    exe: &'a ffi::OsStr,
    cmdline: &'b Vec<u16>,
    cwd: &'c ffi::OsStr,
    env: Vec<u16>,
    stdin: Option<HANDLE>,
    stdout: Option<HANDLE>,
    stderr: Option<HANDLE>,
    allowed_handles: &'d [HANDLE], // stdin/out/err + any extras
) -> windows::core::Result<ProcessInfo> {
    unsafe {
        // ---------------------------
        // Create restricted token
        let mut h_process_token = HANDLE::default();
        Threading::OpenProcessToken( // derive restrictions from the current process.
            Threading::GetCurrentProcess(),
            Security::TOKEN_ALL_ACCESS,
            &mut h_process_token,
        )?;

        let mut h_restricted = HANDLE::default();
        // Minimal: DISABLE_MAX_PRIVILEGE. You can also pass SIDs/privileges lists.
        Security::CreateRestrictedToken(
            h_process_token,
            Security::DISABLE_MAX_PRIVILEGE, // strips *all* privileges from the new token.
            None, // no explicit disabled SIDs, which avoids breaking compatibility for things like DLL loading during group access.
            None, // no explicit privilege list (all are already stripped by DISABLE_MAX_PRIVILEGE)
            None, // no restricting SIDs (we will move to tightening this later; misconfiguring it can break things easily)
            &mut h_restricted
        )?;
        CloseHandle(h_process_token)?;

        // ---------------------------
        // Build STARTUPINFOEX + attribute list
        // First call: get the expected size.
        let mut attr_size: usize = 0;
        match Threading::InitializeProcThreadAttributeList(
            None, // query buffer size
            1, // number of attributes to set 
            Some(0), // must be 0
            &mut attr_size, // output required size in bytes
        ) {
            Ok(()) => (), // unexpected but treat as valid.
            Err(e) => {
                let last_err = GetLastError();
                if last_err != ERROR_INSUFFICIENT_BUFFER {
                    // Real failure.
                    return Err(e);
                }
            }
        }

        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr().cast::<_>());

        Threading::InitializeProcThreadAttributeList(
            Some(attr_list), // allocated buffer
            1, // matches number of attributes to set
            Some(0),
            &mut attr_size,
        )?;

        // Attribute: PROC_THREAD_ATTRIBUTE_HANDLE_LIST
        // This is what ensures ONLY these handles are inherited by the child.
        // To help stabalize this call, the allowed handles is changed into a vector.
        if ! allowed_handles.is_empty() {
            let allowed_handles = allowed_handles.to_vec();
            let cb_size = allowed_handles.len() * mem::size_of::<HANDLE>();
            let lp_value = allowed_handles.as_ptr() as *const core::ffi::c_void;
            Threading::UpdateProcThreadAttribute(
                attr_list, // attribute list
                0, // dwFlags must be 0
                Threading::PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(lp_value),
                cb_size,
                None, // not used
                None, // not used
            )?;
        }

        // ---------------------------
        // STARTUPINFOEX with std handles (these must be in allowed_handles)
        let mut si_ex: Threading::STARTUPINFOEXW = mem::zeroed();
        si_ex.StartupInfo.cb = mem::size_of::<Threading::STARTUPINFOEXW>() as u32;
        si_ex.lpAttributeList = attr_list;

        // Set the std* inputs
        si_ex.StartupInfo.dwFlags = Threading::STARTF_USESTDHANDLES;
        match stdin {
            None => (),
            Some(v) => { si_ex.StartupInfo.hStdInput = v; }
        }
        match stdout {
            None => (),
            Some(v) => { si_ex.StartupInfo.hStdOutput = v; }
        }
        match stderr {
            None => (),
            Some(v) => { si_ex.StartupInfo.hStdError = v; }
        }

        let mut pi: Threading::PROCESS_INFORMATION = mem::zeroed();

        let app = as_c_str_w(exe);
        let cwd = as_c_str_w(cwd);

        // ---------------------------
        // CreateProcessAsUser with restricted token
        Threading::CreateProcessAsUserW(
            Some(h_restricted), // child restricted token
            windows::core::PCWSTR(app.as_ptr()), // application name
            Some(windows::core::PWSTR(cmdline.clone().as_mut_ptr())), // command line
            None, // process attributes
            None, // thread attributes
            // handle inheritance behavior is controlled by the attribute list;
            // the handle-list is the *explicit* allowlist gate.
            true, // must be true to allow handle inheritance to occur at all.
            Threading::CREATE_SUSPENDED // start suspended to allow job assignment before execution
            | Threading::EXTENDED_STARTUPINFO_PRESENT // use extended startup information
            | Threading::CREATE_UNICODE_ENVIRONMENT // set the environment using unicode
            , 
            Some(env.as_ptr() as *const ffi::c_void), // set the environment explicitly
            windows::core::PCWSTR(cwd.as_ptr()), // set the current directory
            &si_ex.StartupInfo, // STARTUPINFOEXW
            &mut pi, // process information
        )?;

        // Token no longer needed
        CloseHandle(h_restricted)?;

        // ---------------------------
        // Put process in a job object with strong limits
        let job = JobObjects::CreateJobObjectW(None, windows::core::PCWSTR::null())?;

        let mut basic: JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION = mem::zeroed();
        basic.LimitFlags = JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JobObjects::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        basic.ActiveProcessLimit = 1;

        let mut ext: JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        ext.BasicLimitInformation = basic;

        JobObjects::SetInformationJobObject(
            job,
            JobObjects::JobObjectExtendedLimitInformation,
            &mut ext as *mut _ as *mut _,
            mem::size_of::<JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;

        JobObjects::AssignProcessToJobObject(job, pi.hProcess)?;

        // ---------------------------
        // Resume thread to allow the process to start, and clean up
        Threading::ResumeThread(pi.hThread);

        // Cleanup attribute list
        Threading::DeleteProcThreadAttributeList(attr_list);

        Ok(ProcessInfo{ process: pi.hProcess, thread: pi.hThread, job })
    }
}

/// Convert the OS string into a null-terminated wide (16-bit) C string.
fn as_c_str_w(s: &ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}
