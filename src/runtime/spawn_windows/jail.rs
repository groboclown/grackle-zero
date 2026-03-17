// SPDX-License-Identifier: MIT


use super::appcontainer::AppContainer;
use super::attribute_list::{
    ThreadAttributeHandles, ThreadAttributeList, ThreadAttributeSecurityCapabilities, policy_flags,
    ThreadAttributeMitigationPolicy, ThreadAttributeMitigationPolicyFlag, ThreadAttributeChildProcessRestriction, NO_CHILD_PROCESS_RESTRICTION,
};
use super::conv::as_c_str_w;
use super::desktop::UiIsolate;
use super::error::WindowsSandboxError;
use std::{collections::HashMap, ffi, mem};
use windows::Win32::{
    Foundation::HANDLE,
    System::{JobObjects, Threading},
};

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
        match stdin {
            Some(h) => {
                allowed_handles.push(h);
            }
            None => (),
        };
        match stdout {
            Some(h) => {
                allowed_handles.push(h);
            }
            None => (),
        };
        match stderr {
            Some(h) => {
                allowed_handles.push(h);
            }
            None => (),
        };
        if allowed_handles.len() <= 0 {
            // If allowed_handles is empty, then the call to add the handles
            // to the attribute list fails, because that only allows the call if there
            // are more than 1 handle to pass.  Rather than add a bunch of conditional
            // logic around the number of attributes, just require 1.  Note that,
            // without at least 1 handle, no communication to the child process is possible,
            // and it has no practical purpose other than spin CPU time.
            return Err(WindowsSandboxError::setup_message(
                "must have at least 1 handle",
            ));
        }

        // ---------------------------
        // Create restricted token
        // In the AppContainer context, the restricted token doesn't work as expected.
        let mut h_process_token = super::process_token::ProcessToken::current_process()?;
        let mut h_restricted = h_process_token.create_restricted_token()?;
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
            Box::new(NO_CHILD_PROCESS_RESTRICTION as ThreadAttributeChildProcessRestriction),
            Box::<ThreadAttributeMitigationPolicyFlag>::new(ThreadAttributeMitigationPolicy::slice(&[
                // Only allow microsoft signed binaries.  This includes the executable, so
                // don't include it.
                // policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_ON,

                // Disable win32k system calls, which prevents a large class of syscalls related to UI and GDI.
                // However, this also prevents any application that uses user32.dll, which is basically all
                // applications, GUI or otherwise.  It also means no stdio calls, so it only works with inherited
                // handles.
                //policy_flags::PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON,

                // Disable extension points, which prevents a large class of DLL injection and code execution techniques.
                // policy_flags::PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON,

                // Enable Data Execution Prevention (DEP) to prevent execution of code from non-executable memory regions.
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE,

                // Enable SEHOP to prevent exploitation of structured exception handling vulnerabilities.
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_SEHOP_ENABLE,

                // Enable heap termination to prevent exploitation of heap vulnerabilities.
                //policy_flags::PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_ON,

                // Optional.  This prevents the process from ever being able to generate code at runtime,
                // which is a common technique for exploits.  However, this also prevents JIT compilers from working,
                // so it may cause compatibility issues with some applications.
                //policy_flags::PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON,

                // Because UI is disabled, there's no reason to allow fonts.
                //policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_ON,

                // Forcibly rebases images that are not dynamic base compatible by acting as though an image base collision happened at load time.
                // The more restrictive mode, ALWAYS_ON_REQ_RELOCS, is too restrictive for default.
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON,

                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON,
            ]).into()),
        ])?;

        // Set up UI isolation.
        let ui_isolate = UiIsolate::initialize("grackle-zero-desktop")?;

        // STARTUPINFOEX with std handles (these must be in allowed_handles)
        let mut si_ex: Threading::STARTUPINFOEXW = mem::zeroed();
        si_ex.StartupInfo.cb = mem::size_of::<Threading::STARTUPINFOEXW>() as u32;
        si_ex.StartupInfo.lpDesktop = ui_isolate.lp_desktop();
        si_ex.lpAttributeList = attributes.list();

        // Set the std* inputs
        si_ex.StartupInfo.dwFlags = Threading::STARTF_USESTDHANDLES;
        match stdin {
            None => (),
            Some(v) => {
                si_ex.StartupInfo.hStdInput = v;
            }
        }
        match stdout {
            None => (),
            Some(v) => {
                si_ex.StartupInfo.hStdOutput = v;
            }
        }
        match stderr {
            None => (),
            Some(v) => {
                si_ex.StartupInfo.hStdError = v;
            }
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
            h_restricted.handle(),                                    // restricted token
            windows::core::PCWSTR(app.as_ptr()),                      // application name
            Some(windows::core::PWSTR(cmdline.clone().as_mut_ptr())), // command line
            None,                                                     // process attributes
            None,                                                     // thread attributes
            // handle inheritance behavior is controlled by the attribute list;
            // the handle-list is the *explicit* allowlist gate.
            true, // must be true to allow handle inheritance to occur at all.
            Threading::CREATE_SUSPENDED // start suspended to allow job assignment before execution
                | Threading::EXTENDED_STARTUPINFO_PRESENT // use extended startup information
                | Threading::CREATE_UNICODE_ENVIRONMENT, // set the environment using unicode
            Some(env.as_ptr() as *const ffi::c_void), // set the environment explicitly
            windows::core::PCWSTR(cwd.as_ptr()), // set the current directory
            &si_ex.StartupInfo, // STARTUPINFOEXW
            &mut pi, // output process information
        )?;

        // Token no longer needed
        h_restricted.close()?;

        // ---------------------------
        // Put process in a job object with strong limits
        let job = JobObjects::CreateJobObjectW(None, windows::core::PCWSTR::null())?;

        let mut basic: JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION = mem::zeroed();
        basic.LimitFlags = JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JobObjects::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
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

        // TODO inject ntdll patching + inline syscall trampoline.
        // This requires:
        //   1. allocating memory in the process (VirtualEllocEx(pi.hProcess, ...))
        //   2. writing the trampoline code into that memory (WriteProcessMemory(pi.hProcess, addr, wide_dll_path, ...))
        //   3. creating a remote thread to execute the trampoline (CreateRemoteThread(pi.hProcess, NULL, 0, LoadLibraryW, addr, 0, NULL))
        //   4. Wait for the loader thread to finish.  This should do something like exposing a named event or a completion protocol.

        // ---------------------------
        // Resume thread to allow the process to start, and clean up
        Threading::ResumeThread(pi.hThread);

        // The other structures will drop their handles when this function returns.

        Ok(ProcessInfo {
            process: pi.hProcess,
            thread: pi.hThread,
            job,
        })
    }
}

fn with_default_environ(
    app: &AppContainer,
    mut environ: HashMap<ffi::OsString, ffi::OsString>,
) -> Result<Vec<u16>, WindowsSandboxError> {
    let system_root = std::env::var_os("SYSTEMROOT").unwrap_or_else(|| ffi::OsString::new());

    // If SYSTEMROOT is not set, add it from the current process's environment.
    if !environ
        .iter()
        .any(|(k, _)| k.to_string_lossy().to_uppercase() == "SYSTEMROOT")
    {
        environ.insert(ffi::OsString::from("SystemRoot"), system_root.clone());
    }
    // ... same for winroot.
    if !environ
        .iter()
        .any(|(k, _)| k.to_string_lossy().to_uppercase() == "WINDIR")
    {
        environ.insert(
            ffi::OsString::from("Windir"),
            std::env::var_os("WINDIR").unwrap_or_else(|| ffi::OsString::new()),
        );
    }

    // Use a minimal path, if not given.
    if !environ
        .iter()
        .any(|(k, _)| k.to_string_lossy().to_uppercase() == "PATH")
    {
        let mut path = ffi::OsString::from(&system_root);
        path.push(";");
        path.push(&system_root);
        path.push("\\System32");
        environ.insert(ffi::OsString::from("Path"), path);
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
        environ
            .into_iter()
            .collect::<Vec<(ffi::OsString, ffi::OsString)>>()
            .as_slice(),
    )
    .map_err(|e| WindowsSandboxError::Sandbox(e))
}
