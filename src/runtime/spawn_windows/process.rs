// SPDX-License-Identifier: MIT

use std::{ffi::OsStr, mem, os::windows::ffi::OsStrExt, ptr, sync::{Arc, Mutex}};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security,
        System::{JobObjects, Threading},
    },
    core,
};


#[derive(Clone)]
struct ProcessState {
    inner: Arc<Mutex<InnerProcessState>>,
}

impl ProcessState {
    fn new(process: HANDLE, thread: HANDLE, job: HANDLE) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerProcessState { process, thread, job })),
        }
    }

    fn access<R, F>(&self, f: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(&mut InnerProcessState) -> R,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        Ok(f(&mut *guard))
    }
}


struct InnerProcessState {
    process: HANDLE,
    thread: HANDLE,
    job: HANDLE,
}



struct ProcessInfo {
    process: HANDLE,
    thread: HANDLE,
    job: HANDLE,
}

/// Spawn the executable in a restricted mode.
/// The handles must be created as inheritable, or run
/// `SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)` on them.
/// Note that all other handles must be cleared of this.
/// Note that passing arguments to the command MUST use proper Windows escaping.  This is a
/// common source of bugs in applications.
fn spawn_restricted(
    exe: &str,
    cmdline: &str,
    allowed_handles: &[HANDLE], // stdin/out/err + any extras
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
        CloseHandle(h_process_token);

        // ---------------------------
        // Build STARTUPINFOEX + attribute list
        let mut attr_size: usize = 0;
        Threading::InitializeProcThreadAttributeList(
            None, // query buffer size
            1, // number of attributes to set 
            Some(0), // must be 0
            &mut attr_size, // output required size in bytes
        );

        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = attr_buf.as_mut_ptr() as Threading::LPPROC_THREAD_ATTRIBUTE_LIST;

        Threading::InitializeProcThreadAttributeList(
            attr_list, // allocated buffer
            1, // matches number of attributes to set
            Some(0),
            &mut attr_size,
        )?;

        // Attribute: PROC_THREAD_ATTRIBUTE_HANDLE_LIST
        // This is what ensures ONLY these handles are inherited by the child.
        Threading::UpdateProcThreadAttribute(
            attr_list, // attribute list
            0, // dwFlags must be 0
            Threading::PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            allowed_handles.as_ptr() as *const core::ffi::c_void,
            allowed_handles.len() * mem::size_of::<HANDLE>(),
            None, // not used
            None, // not used
        )?;

        // ---------------------------
        // STARTUPINFOEX with std handles (these must be in allowed_handles)
        let mut si_ex: Threading::STARTUPINFOEXW = mem::zeroed();
        si_ex.StartupInfo.cb = mem::size_of::<Threading::STARTUPINFOEXW>() as u32;
        si_ex.lpAttributeList = attr_list;

        // TODO To set the std* inputs, they must be passed in like this:
        // si_ex.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        // si_ex.StartupInfo.hStdInput = stdin;
        // si_ex.StartupInfo.hStdOutput = stdout;
        // si_ex.StartupInfo.hStdError = stderr;

        let mut pi: Threading::PROCESS_INFORMATION = mem::zeroed();

        let app = widestr(exe);
        let mut cmd = widestr(cmdline);

        // ---------------------------
        // CreateProcessAsUser with restricted token
        Threading::CreateProcessAsUserW(
            Some(h_restricted), // child restricted token
            core::PCWSTR(app.as_ptr()), // application name
            Some(core::PWSTR(cmd.as_mut_ptr())), // command line
            None, // process attributes
            None, // thread attributes
            // handle inheritance behavior is controlled by the attribute list;
            // the handle-list is the *explicit* allowlist gate. :contentReference[oaicite:7]{index=7}
            true, // must be true to allow handle inheritance to occur at all.
            Threading::CREATE_SUSPENDED | Threading::EXTENDED_STARTUPINFO_PRESENT, // start suspended to allow job assignment before execution
            None, // TODO set the environment explicitly; setting to None inherits from parent.
            core::PCWSTR(ptr::null()), // TODO set the current directory; null means inherit from the parent.
            &si_ex.StartupInfo, // STARTUPINFOEXW
            &mut pi, // process information
        )?;

        // Token no longer needed
        CloseHandle(h_restricted);

        // ---------------------------
        // Put process in a job object with strong limits
        let job = JobObjects::CreateJobObjectW(None, core::PCWSTR(ptr::null()))?;

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

fn widestr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
