// SPDX-License-Identifier: MIT
use std::{
    collections::HashMap,
    ffi, mem,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use windows::Win32::{
    Foundation::HANDLE,
    System::{JobObjects, Threading},
};

use crate::restrictions;

use super::appcontainer::AppContainer;
use super::attribute_list::{
    NO_CHILD_PROCESS_RESTRICTION, ThreadAttribute, ThreadAttributeChildProcessRestriction,
    ThreadAttributeHandles, ThreadAttributeList, ThreadAttributeMitigationPolicy,
    ThreadAttributeMitigationPolicyFlag, new_appcontainer_attribute, policy_flags,
};
use super::conv::{as_c_str_w, c_str_w_as_str};
use super::desktop::UiIsolate;
use super::error::WindowsSandboxError;

#[derive(Clone)]
pub struct ProcessInfo {
    pub process: HANDLE,
    pub thread: HANDLE,
    pub job: HANDLE,
    // Keep UI isolation objects alive while the process state is held by callers.
    // Dropping these too early can tear down the child desktop/window station
    // during startup.
    _ui_isolate: Arc<UiIsolate>,
}

static LAUNCH_SEQ: AtomicU64 = AtomicU64::new(1);

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
    env: HashMap<ffi::OsString, ffi::OsString>,
    stdin: Option<HANDLE>,
    stdout: Option<HANDLE>,
    stderr: Option<HANDLE>,
    allowed_handles: &'d [HANDLE], // stdin/out/err + any extras
    restr: &restrictions::Restrictions,
) -> Result<ProcessInfo, WindowsSandboxError> {
    unsafe {
        let launch_id = LAUNCH_SEQ.fetch_add(1, Ordering::Relaxed);
        // ---------------------------
        // Pre-condition check.
        let mut allowed_handles = allowed_handles.to_vec();
        allowed_handles = add_std_handle(allowed_handles, stdin, restr)?;
        allowed_handles = add_std_handle(allowed_handles, stdout, restr)?;
        allowed_handles = add_std_handle(allowed_handles, stderr, restr)?;

        // Note that, without at least 1 handle, no communication to the child process is possible,
        // and it has no practical purpose other than spin CPU time.

        // ---------------------------
        // Prepare the AppContainer.
        let appcontainer = match AppContainer::new(restr) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: AppContainer::new failed: {:?}",
                    e
                );
                return Err(e);
            }
        };

        // ---------------------------
        // Build STARTUPINFOEX + attribute list
        let mut attributes: Vec<Box<dyn ThreadAttribute>> = vec![
            // Prohibit launching a child process.  Always active.
            Box::new(NO_CHILD_PROCESS_RESTRICTION as ThreadAttributeChildProcessRestriction),
        ];
        let has_allowed_handles = !allowed_handles.is_empty();
        if has_allowed_handles {
            // Allow the child process to access the allowed handles list.
            attributes.push(Box::new(allowed_handles as ThreadAttributeHandles));
        }
        if let Some(sid) = appcontainer.sid() {
            // Security Capabilities tells CreateProcess to create an AppContainer token.
            // Capabilities = none => no file/network capabilities beyond the default container allowances.
            attributes.push(Box::new(new_appcontainer_attribute(sid)));
        }
        let mitigation = generate_mitigation_policy_flags(restr);
        attributes.push(Box::new(ThreadAttributeMitigationPolicy::new(
            mitigation.policy,
            mitigation.policy2,
        )));
        let attributes = match ThreadAttributeList::new(attributes) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: ThreadAttributeList::new failed: {:?}",
                    e
                );
                return Err(e);
            }
        };

        // Set up UI isolation.
        let ui_isolate = match UiIsolate::initialize(restr, appcontainer.sid()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: UiIsolate::initialize failed: {:?}",
                    e
                );
                return Err(e);
            }
        };

        // STARTUPINFOEX with std handles (these must be in allowed_handles)
        let mut si_ex: Threading::STARTUPINFOEXW = mem::zeroed();
        si_ex.StartupInfo.cb = mem::size_of::<Threading::STARTUPINFOEXW>() as u32;

        si_ex.StartupInfo.lpDesktop = ui_isolate.lp_desktop();

        si_ex.lpAttributeList = attributes.list();

        // Set stdio fields only when at least one stdio handle is explicitly configured.
        // Leaving STARTF_USESTDHANDLES off avoids forcing null/invalid std handles.
        let use_std_handles = stdin.is_some() || stdout.is_some() || stderr.is_some();
        if use_std_handles {
            si_ex.StartupInfo.dwFlags |= Threading::STARTF_USESTDHANDLES;
        }
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

        let app = as_c_str_w(exe);
        let mut cwd = match app_container_cwd(&appcontainer, launch_id) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: app_container_cwd failed: {:?}",
                    e
                );
                return Err(e);
            }
        };
        let env = match with_default_environ(&appcontainer, env) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: with_default_environ failed: {:?}",
                    e
                );
                return Err(e);
            }
        };
        ensure_valid_launch_cwd(&appcontainer, launch_id, &mut cwd)?;
        let mut pi: Threading::PROCESS_INFORMATION = mem::zeroed();
        let mut cmdline_buf = cmdline.clone();
        let creation_flags = Threading::CREATE_SUSPENDED // start suspended to allow job assignment before execution
            | Threading::EXTENDED_STARTUPINFO_PRESENT // use extended startup information
            | Threading::CREATE_UNICODE_ENVIRONMENT; // set the environment using unicode

        if let Err(e) = appcontainer.create_child_process(
            app.as_slice(),
            cmdline_buf.as_mut_slice(),
            // handle inheritance behavior is controlled by the attribute list;
            // the handle-list is the explicit allowlist gate.
            has_allowed_handles, // true only when using PROC_THREAD_ATTRIBUTE_HANDLE_LIST
            creation_flags,
            env.as_ptr() as *const ffi::c_void,
            cwd.as_slice(),
            &si_ex.StartupInfo,
            &mut pi,
        ) {
            eprintln!(
                "[launch {launch_id}] launch_restricted: process creation failed: {:?}",
                e
            );
            return Err(e);
        }

        // ---------------------------
        // Put process in a job object with strong limits
        let job = match JobObjects::CreateJobObjectW(None, windows::core::PCWSTR::null()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "[launch {launch_id}] launch_restricted: CreateJobObjectW failed: {:?}",
                    e
                );
                return Err(e.into());
            }
        };

        let mut basic: JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION = mem::zeroed();
        basic.LimitFlags = JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JobObjects::JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        basic.ActiveProcessLimit = 1;

        let mut ext: JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        ext.BasicLimitInformation = basic;

        if let Err(e) = JobObjects::SetInformationJobObject(
            job,
            JobObjects::JobObjectExtendedLimitInformation,
            &mut ext as *mut _ as *mut _,
            mem::size_of::<JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) {
            eprintln!(
                "[launch {launch_id}] launch_restricted: SetInformationJobObject failed: {:?}",
                e
            );
            return Err(e.into());
        }

        if let Err(e) = JobObjects::AssignProcessToJobObject(job, pi.hProcess) {
            eprintln!(
                "[launch {launch_id}] launch_restricted: AssignProcessToJobObject failed: {:?}",
                e
            );
            return Err(e.into());
        }

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
            _ui_isolate: Arc::new(ui_isolate),
        })
    }
}

fn app_container_cwd(app: &AppContainer, launch_id: u64) -> Result<Vec<u16>, WindowsSandboxError> {
    // If the restrictions didn't create an app container, then this needs to return the active
    // user's temporary folder.
    match app.sid() {
        None => Ok(super::conv::as_c_str_w(std::env::temp_dir().as_os_str())),
        Some(_) => {
            let mut folder = app.folder_path()?;
            folder.push("\\Temp");
            let dir_path = std::path::PathBuf::from(&folder);

            if let Err(create_err) = std::fs::create_dir_all(&dir_path) {
                // Keep an explicit post-check so callers are robust even when
                // create_dir_all races or returns spurious errors.
                match std::fs::metadata(&dir_path) {
                    Ok(meta) if meta.is_dir() => (),
                    Ok(_) => {
                        return Err(WindowsSandboxError::setup_message(
                            "app container temp folder is not a directory",
                        ));
                    }
                    Err(_) => {
                        eprintln!(
                            "[launch {launch_id}] app_container_cwd create_dir_all failed path={:?} err={}",
                            dir_path, create_err
                        );
                        return Err(WindowsSandboxError::setup_message(&format!(
                            "failed to create app container temp folder: {}",
                            create_err
                        )));
                    }
                }
            }

            match std::fs::metadata(&dir_path) {
                Ok(meta) if meta.is_dir() => Ok(super::conv::as_c_str_w(&folder)),
                Ok(_) => Err(WindowsSandboxError::setup_message(
                    "app container temp folder is not a directory",
                )),
                Err(metadata_err) => Err(WindowsSandboxError::setup_message(&format!(
                    "failed to inspect app container temp folder: {}",
                    metadata_err
                ))),
            }
        }
    }
}

fn ensure_valid_launch_cwd(
    app: &AppContainer,
    launch_id: u64,
    cwd: &mut Vec<u16>,
) -> Result<(), WindowsSandboxError> {
    let cwd_path = PathBuf::from(c_str_w_as_str(cwd.as_slice()));
    if let Ok(meta) = std::fs::metadata(&cwd_path) {
        if meta.is_dir() {
            return Ok(());
        }
        return Err(WindowsSandboxError::setup_message(
            "configured launch cwd exists but is not a directory",
        ));
    }

    *cwd = app_container_cwd(app, launch_id)?;
    let rebuilt_path = PathBuf::from(c_str_w_as_str(cwd.as_slice()));
    match std::fs::metadata(&rebuilt_path) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(WindowsSandboxError::setup_message(
            "rebuilt launch cwd exists but is not a directory",
        )),
        Err(e) => Err(WindowsSandboxError::setup_message(&format!(
            "failed to inspect launch cwd after rebuild: {}",
            e
        ))),
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

    // Force the AppContainer profile folders or user version.
    let (app_folder, tmp_folder) = match app.sid() {
        None => {
            let mut oss = std::ffi::OsString::new();
            oss.push(std::env::temp_dir().as_os_str());
            (oss.clone(), oss)
        }
        Some(_) => {
            let f = app.folder_path()?;
            let mut t = f.clone();
            t.push("\\Temp");
            (f, t)
        }
    };
    environ.insert("LOCALAPPDATA".into(), app_folder.clone().into());
    environ.insert("TEMP".into(), tmp_folder.clone().into());
    environ.insert("TMP".into(), tmp_folder.into());

    // Windows requires a hidden drive-current-directory entry when a CWD is passed
    // to CreateProcess*. Without this, process creation can intermittently fail
    // with "The directory name is invalid" in some contexts.
    let temp_path = std::path::PathBuf::from(&app_folder);
    if let Some(prefix) = temp_path.as_path().components().next() {
        if let std::path::Component::Prefix(prefix_component) = prefix {
            if let std::path::Prefix::Disk(drive) = prefix_component.kind() {
                let drive = (drive as char).to_ascii_uppercase();
                let mut drive_key = ffi::OsString::new();
                drive_key.push(format!("={}:", drive).as_str());
                environ.insert(drive_key, app_folder.clone().into());
            }
        }
    }

    super::launch_quote::encode_env_strings(
        environ
            .into_iter()
            .collect::<Vec<(ffi::OsString, ffi::OsString)>>()
            .as_slice(),
    )
    .map_err(|e| WindowsSandboxError::Sandbox(e))
}

fn add_std_handle(
    mut handles: Vec<HANDLE>,
    handle: Option<HANDLE>,
    restr: &restrictions::Restrictions,
) -> Result<Vec<HANDLE>, WindowsSandboxError> {
    match handle {
        Some(h) => {
            if matches!(
                restr.windows.disable_win32k_system_calls,
                restrictions::windows::AlwaysMode::AlwaysOn
            ) {
                return Err(WindowsSandboxError::setup_message(
                    "cannot use CLI std* handles with win32k system call restriction",
                ));
            }
            handles.push(h);
            Ok(handles)
        }
        None => Ok(handles),
    }
}

struct MitigationPolicies {
    policy: ThreadAttributeMitigationPolicyFlag,
    policy2: ThreadAttributeMitigationPolicyFlag,
}

#[derive(Clone, Copy, Debug)]
struct WindowsVersion {
    major: u32,
    minor: u32,
    build: u32,
}

impl WindowsVersion {
    fn is_at_least(self, major: u32, minor: u32, build: u32) -> bool {
        if self.major != major {
            return self.major > major;
        }
        if self.minor != minor {
            return self.minor > minor;
        }
        self.build >= build
    }
}

fn current_windows_version() -> Option<WindowsVersion> {
    let mut info = windows::Win32::System::SystemInformation::OSVERSIONINFOW::default();
    info.dwOSVersionInfoSize =
        std::mem::size_of::<windows::Win32::System::SystemInformation::OSVERSIONINFOW>() as u32;
    let status =
        unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut info as *mut _) };
    if status.0 < 0 {
        return None;
    }
    Some(WindowsVersion {
        major: info.dwMajorVersion,
        minor: info.dwMinorVersion,
        build: info.dwBuildNumber,
    })
}

fn generate_mitigation_policy_flags(restr: &restrictions::Restrictions) -> MitigationPolicies {
    // Per restrictions.rs + UpdateProcThreadAttribute docs:
    // - Windows 7+: DEP, SEHOP
    // - Windows 8+: most PROCESS_CREATION_MITIGATION_POLICY values
    // - Windows 10 1709+: POLICY2_RESTRICT_INDIRECT_BRANCH_PREDICTION
    // - Windows 10 1809+: POLICY2_SPECULATIVE_STORE_BYPASS_DISABLE
    // - Windows 10 2004+: CET/POLICY2 remainder (including FSCTL disable)
    let ver = current_windows_version().unwrap_or(WindowsVersion {
        major: 6,
        minor: 1,
        build: 0,
    });
    let supports_win8_policy = ver.is_at_least(6, 2, 0);
    let supports_win10_1709 = ver.is_at_least(10, 0, 16299);
    let supports_win10_1809 = ver.is_at_least(10, 0, 17763);
    let supports_win10_2004 = ver.is_at_least(10, 0, 19041);

    let mut policy: ThreadAttributeMitigationPolicyFlag = 0;
    let mut policy2: ThreadAttributeMitigationPolicyFlag = 0;
    match restr.windows.data_execution_prevention {
        restrictions::windows::DataExecutionPreventionMode::Disabled => (),
        restrictions::windows::DataExecutionPreventionMode::Enabled => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE;
        }
        restrictions::windows::DataExecutionPreventionMode::ThunkEmulation => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE;
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_DEP_ATL_THUNK_ENABLE;
        }
    }

    match restr
        .windows
        .structured_exception_handler_overwrite_protection
    {
        restrictions::windows::RestrictedAlwaysMode::Defer => (),
        restrictions::windows::RestrictedAlwaysMode::AlwaysOn => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_SEHOP_ENABLE;
        }
    }

    // Ordered by earliest version to latest version.
    // If you don't have the first one, then later ones don't apply.

    // ----------------------------------------------------------------
    if !supports_win8_policy {
        return MitigationPolicies { policy, policy2 };
    }

    match restr.windows.aslr.force_enabled {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_OFF;
        }
    }
    match restr.windows.aslr.heap_terminate_on_corruption {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_OFF;
        }
    }
    if restr.windows.aslr.require_relocation {
        policy |=
            policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON_REQ_RELOCS;
    }
    match restr.windows.aslr.bottom_up_randomization {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_OFF;
        }
    }
    match restr.windows.aslr.high_entropy_randomization {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_OFF;
        }
    }

    match restr.windows.strict_handle_checking {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_OFF;
        }
    }

    match restr.windows.disable_win32k_system_calls {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_OFF;
        }
    }

    match restr.windows.disable_extension_points {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_OFF;
        }
    }

    match &restr.windows.control_flow_guard {
        restrictions::windows::ControlFlowGuardPolicy::Defer => (),
        restrictions::windows::ControlFlowGuardPolicy::Enable(settings) => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_ON;
            match &settings.require_cfg_images {
                restrictions::windows::AlwaysMode::Defer => (),
                restrictions::windows::AlwaysMode::AlwaysOn => {
                    policy2 |=
                        policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_ON;
                }
                restrictions::windows::AlwaysMode::AlwaysOff => {
                    policy2 |=
                        policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_OFF;
                }
            }
            if settings.export_suppression {
                policy |=
                    policy_flags::PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_EXPORT_SUPPRESSION;
            }
        }
    }

    match restr.windows.dynamic_code {
        restrictions::windows::DynamicCodePolicy::Defer => (),
        restrictions::windows::DynamicCodePolicy::AlwaysProhibit => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON;
        }
        restrictions::windows::DynamicCodePolicy::AllowOptOut => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON_ALLOW_OPT_OUT;
        }
        restrictions::windows::DynamicCodePolicy::AlwaysAllow => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_OFF;
        }
    }

    match restr.windows.binary_signature {
        restrictions::windows::BinarySignaturePolicy::Defer => (),
        restrictions::windows::BinarySignaturePolicy::AllowOnlyMicrosoft => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_ON;
        }
        restrictions::windows::BinarySignaturePolicy::AllowAny => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_OFF;
        }
        restrictions::windows::BinarySignaturePolicy::AllowStore => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALLOW_STORE;
        }
    }

    match restr.windows.font_loading_policy {
        restrictions::windows::FontLoadingPolicy::Defer => (),
        restrictions::windows::FontLoadingPolicy::AlwaysPrevent => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_ON;
        }
        restrictions::windows::FontLoadingPolicy::AlwaysAllow => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_OFF;
        }
        restrictions::windows::FontLoadingPolicy::AuditNonSystemFonts => {
            policy |= policy_flags::PROCESS_CREATION_MITIGATION_POLICY_AUDIT_NONSYSTEM_FONTS;
        }
    }

    match restr.windows.image_load_policy.no_remote {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_OFF;
        }
    }
    match restr.windows.image_load_policy.no_low_label {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_OFF;
        }
    }
    match restr.windows.image_load_policy.prefer_system32 {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_OFF;
        }
    }

    // ----------------------------------------------------------------
    if !supports_win10_1709 {
        return MitigationPolicies { policy, policy2 };
    }

    if restr.windows.restrict_indirect_branch_prediction {
        policy2 |=
            policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_RESTRICT_INDIRECT_BRANCH_PREDICTION_ALWAYS_ON;
    }

    // ----------------------------------------------------------------
    if !supports_win10_1809 {
        return MitigationPolicies { policy, policy2 };
    }

    if restr.windows.disable_speculative_store_bypass {
        policy2 |=
            policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_SPECULATIVE_STORE_BYPASS_DISABLE_ALWAYS_ON;
    }

    // ----------------------------------------------------------------
    if !supports_win10_2004 {
        return MitigationPolicies { policy, policy2 };
    }

    match restr.windows.cet_user_shadow_stack {
        restrictions::windows::CETUserShadowStack::Defer => (),
        restrictions::windows::CETUserShadowStack::AlwaysOn => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_ON;
        }
        restrictions::windows::CETUserShadowStack::AlwaysOff => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_OFF;
        }
        restrictions::windows::CETUserShadowStack::StrictMode => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_STRICT_MODE;
        }
    }

    match restr.windows.cet_context_ip_validation {
        restrictions::windows::CETContextIPValidation::Defer => (),
        restrictions::windows::CETContextIPValidation::AlwaysOn => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_ON;
        }
        restrictions::windows::CETContextIPValidation::AlwaysOff => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_OFF;
        }
        restrictions::windows::CETContextIPValidation::RelaxedMode => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_RELAXED_MODE;
        }
    }

    match restr.windows.cet_binary_load_blocking {
        restrictions::windows::CETBinaryLoadBlocking::Defer => (),
        restrictions::windows::CETBinaryLoadBlocking::AlwaysOn => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_ON;
        }
        restrictions::windows::CETBinaryLoadBlocking::AlwaysOff => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_OFF;
        }
        restrictions::windows::CETBinaryLoadBlocking::BlockNonEHCont => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_NON_EHCONT;
        }
    }

    match restr.windows.cet_dynamic_apis_out_of_proc_only {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_OFF;
        }
    }

    match restr.windows.disable_fsctl_system_call {
        restrictions::windows::AlwaysMode::Defer => (),
        restrictions::windows::AlwaysMode::AlwaysOn => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_ON;
        }
        restrictions::windows::AlwaysMode::AlwaysOff => {
            policy2 |=
                policy_flags::PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_OFF;
        }
    }

    MitigationPolicies { policy, policy2 }
}
