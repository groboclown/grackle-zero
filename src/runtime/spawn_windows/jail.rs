// SPDX-License-Identifier: MIT

use std::{collections::HashMap, ffi, mem};
use windows::Win32::{
    Foundation::HANDLE,
    System::{JobObjects, Threading},
};
use super::error::WindowsSandboxError;
use super::appcontainer::AppContainer;
use super::attribute_list::{ThreadAttributeList, ThreadAttributeHandles, ThreadAttributeSecurityCapabilities};
use super::conv::as_c_str_w;


#[derive(Clone)]
pub struct ProcessInfo {
    pub process: HANDLE,
    pub thread: HANDLE,
    pub job: HANDLE,
}

const APPCONTAINER_NAME: &str = "grackle-zero";

/// Spawn the executable in a restricted mode.
/// Make sure to pass the handles through the `prepare_inheritance_allowlist` function.
/// Construct the cmdline argument with the `launch_quote::quote_arguments` function.
/// Construct the env argument with the `launch_quote::encode_env_strings` function.
/// 
/// The `cmdline` MUST include the exe's full path as the first argument, due to
/// how Windows works with AppContainer.
pub fn launch_restricted<'a, 'b, 'c, 'd>(
    exe: &'a ffi::OsStr,
    cmdline: &'b Vec<u16>,
    cwd: &'c ffi::OsStr,
    env: HashMap<ffi::OsString, ffi::OsString>,
    stdin: Option<HANDLE>,
    stdout: Option<HANDLE>,
    stderr: Option<HANDLE>,
    allowed_handles: &'d [HANDLE], // stdin/out/err + any extras
) -> Result<ProcessInfo, WindowsSandboxError> {
    unsafe {
        // ---------------------------
        // Pre-condition check.
        let mut allowed_handles = allowed_handles.to_vec();
        match stdin { Some(h) => { allowed_handles.push(h); } None => () };
        match stdout { Some(h) => { allowed_handles.push(h); } None => () };
        match stderr { Some(h) => { allowed_handles.push(h); } None => () };
        if allowed_handles.len() <= 0 {
            // If allowed_handles is empty, then the call to add the handles
            // to the attribute list fails, because that only allows the call if there
            // are more than 1 handle to pass.  Rather than add a bunch of conditional
            // logic around the number of attributes, just require 1.  Note that,
            // without at least 1 handle, no communication to the child process is possible,
            // and it has no practical purpose other than spin CPU time.
            return Err(WindowsSandboxError::setup_message("must have at least 1 handle"));
        }

        // ---------------------------
        // Create restricted token
        // In the AppContainer context, the restricted token doesn't work as expected.
        let mut h_process_token = super::process_token::ProcessToken::current_process()?;
        let h_restricted = h_process_token.create_restricted_token()?;
        h_process_token.close()?;

        // ---------------------------
        // Prepare the AppContainer.
        let appcontainer = AppContainer::new(APPCONTAINER_NAME)?;

        // ---------------------------
        // Build STARTUPINFOEX + attribute list
        let attributes = ThreadAttributeList::new(vec![
            // Allow the child process to access the allowed handles list.
            Box::new(allowed_handles as ThreadAttributeHandles),

            // Security Capabilities tells CreateProcess to create an AppContainer token.
            // Capabilities = none => no file/network capabilities beyond the default container allowances.
            Box::new(ThreadAttributeSecurityCapabilities {
                AppContainerSid: appcontainer.sid().expect("appcontainer already dropped"),
                Capabilities: std::ptr::null_mut(),
                CapabilityCount: 0,
                Reserved: 0,
            }),
        ])?;

        // STARTUPINFOEX with std handles (these must be in allowed_handles)
        let mut si_ex: Threading::STARTUPINFOEXW = mem::zeroed();
        si_ex.StartupInfo.cb = mem::size_of::<Threading::STARTUPINFOEXW>() as u32;
        si_ex.lpAttributeList = attributes.list();

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

        // ---------------------------
        // CreateProcessAsUser with restricted token
        // While using the AppContainer *should* be sufficient to run just
        // CreateProcessW (eliminating the need for the child restricted token),
        // let's see if also running with the restricted token also works.
        // This may cause issues with basic DLL loading and Win32 runtime behavior,
        // and may require ACL enabling execution for many objects.

        let app = as_c_str_w(exe);
        let cwd = as_c_str_w(cwd);
        let env = with_default_environ(&appcontainer, env)?;
        let mut pi: Threading::PROCESS_INFORMATION = mem::zeroed();

        //Threading::CreateProcessW(
        Threading::CreateProcessAsUserW(
            h_restricted.handle(), // restricted token
            windows::core::PCWSTR(app.as_ptr()),                  // application name
            Some(windows::core::PWSTR(cmdline.clone().as_mut_ptr())), // command line
            None,                                               // process attributes
            None,                                                // thread attributes
            // handle inheritance behavior is controlled by the attribute list;
            // the handle-list is the *explicit* allowlist gate.
            true, // must be true to allow handle inheritance to occur at all.
                Threading::CREATE_SUSPENDED // start suspended to allow job assignment before execution
                | Threading::EXTENDED_STARTUPINFO_PRESENT // use extended startup information
                | Threading::CREATE_UNICODE_ENVIRONMENT   // set the environment using unicode
            ,
            Some(env.as_ptr() as *const ffi::c_void), // set the environment explicitly
            windows::core::PCWSTR(cwd.as_ptr()), // set the current directory
            &si_ex.StartupInfo, // STARTUPINFOEXW
            &mut pi, // process information
        )?;

        // Token no longer needed
        //h_restricted.close()?;

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

        // The other structures will drop their handles when this function returns.

        Ok(ProcessInfo{ process: pi.hProcess, thread: pi.hThread, job })
    }
}

fn with_default_environ(
    app: &AppContainer,
    mut environ: HashMap<ffi::OsString, ffi::OsString>,
) -> Result<Vec<u16>, WindowsSandboxError> {
    let system_root = std::env::var_os("SYSTEMROOT").unwrap_or_else(|| ffi::OsString::new());

    // If SYSTEMROOT is not set, add it from the current process's environment.
    if !environ.iter().any(|(k, _)| k.to_string_lossy().to_uppercase() == "SYSTEMROOT") {
        environ.insert(
            ffi::OsString::from("SystemRoot"),
            system_root.clone(),
        );
    }
    // ... same for winroot.
    if !environ.iter().any(|(k, _)| k.to_string_lossy().to_uppercase() == "WINDIR") {
        environ.insert(
            ffi::OsString::from("Windir"),
            std::env::var_os("WINDIR").unwrap_or_else(|| ffi::OsString::new()),
        );
    }

    // Use a minimal path, if not given.
    if !environ.iter().any(|(k, _)| k.to_string_lossy().to_uppercase() == "PATH") {
        let mut path = ffi::OsString::from(&system_root);
        path.push(";");
        path.push(&system_root);
        path.push("\\System32");
        environ.insert(
            ffi::OsString::from("Path"),
            path,
        );
    }

    // Force the AppContainer profile folders.
    let app_folder = app.folder_path()?;
    environ.insert("LOCALAPPDATA".into(), app_folder.clone().into());
    let temp_folder = app_folder.clone() + "\\Temp";
    environ.insert("TEMP".into(), temp_folder.clone().into());
    environ.insert("TMP".into(), temp_folder.into());

    // Windows requires the hidden `=C:` only if the CWD is passed to the CreateProcessW,
    // which it is.

    super::launch_quote::encode_env_strings(
        environ.into_iter().collect::<Vec<(ffi::OsString, ffi::OsString)>>().as_slice()
    ).map_err(|e| WindowsSandboxError::Sandbox(e))
}
