// SPDX-License-Identifier: MIT

/// Explicit list of restrictions to apply to the spawned runtime.
/// By default, the system enables all of them.  They must be explicitly disabled if not wanted.
/// Some restrictions apply to a specific operating system or runtime environment.
/// Some restrictions require another restriction in order to run.
#[derive(Debug, Clone, PartialEq)]
pub struct Restrictions {
    pub linux: linux::LinuxRestrictions,
    pub windows: windows::WindowsRestrictions,
}

/// Create the default restrictions, compatible across upgrades.
/// Intended for compatibility across version upgrades.  As the library adds new restrictions,
/// using this invocation should allow the execution of previously working executables to continue to run.
/// This may mean a less restrictive environment than what the library may allow for, but allows for
/// upgrading without needing to rerun expensive compatibility testing for every new version.
pub fn create_compat_restrictions(application_name: &String) -> Restrictions {
    Restrictions {
        linux: linux::compatible_linux_restrictions(),
        windows: windows::compatible_windows_restrictions(application_name),
    }
}

/// Create the default restrictions, with enforced strictness.
/// This allows for upgrading new versions and taking advantage of newly discovered restrictions.
/// This has the downside that some executables that used to work may no longer work as expected
/// after upgrading this library.
pub fn create_strict_restrictions(application_name: &String) -> Restrictions {
    Restrictions {
        linux: linux::strict_linux_restrictions(),
        windows: windows::strict_windows_restrictions(application_name),
    }
}

mod tests {
    // Not sure why Rust marks these as unused, when they're clearly used in the tests,
    // and removing these causes errors.
    #[allow(unused)]
    use super::{linux, windows};
    #[allow(unused)]
    use crate::{compat_restrictions, strict_restrictions};

    #[test]
    fn test_strict_restrictions() {
        let r = strict_restrictions!(
            "test_app",
            |r| { linux::with_max_open_files(r, 4096) },
            linux::kill_process_on_seccomp_violation,
            windows::disable_desktop_isolation,
            windows::disable_data_execution_prevention,
            (
                windows::with_app_container_capability,
                windows::AppContainerCapability::Microphone,
            ),
            |r| {
                windows::with_app_container_capability(r, windows::AppContainerCapability::Webcam)
            },
            windows::defer_aslr_policy_forced,
        );
        assert_eq!(r.linux.max_open_files, 4096);
        assert_eq!(r.linux.secomp_kill, true);
        let app_container = match r.windows.app_container {
            windows::AppContainerMode::Enabled(a) => a,
            windows::AppContainerMode::Disabled => {
                panic!("Created a disabled app container");
            }
        };
        assert_eq!(
            app_container.capabilities,
            vec![
                windows::AppContainerCapability::Microphone,
                windows::AppContainerCapability::Webcam
            ]
        );
    }

    #[test]
    fn test_compat_restrictions() {
        let r = compat_restrictions!(
            "test_app",
            (
                linux::with_max_open_files,
                300,
            ),
            windows::disable_app_container,
            windows::disable_desktop_isolation,
            windows::disable_data_execution_prevention,
            |r| {
                windows::with_app_container_capability(r, windows::AppContainerCapability::Webcam)
            },
            windows::defer_aslr_policy_forced,
        );
        match r.windows.app_container {
            windows::AppContainerMode::Enabled(_) => {
                panic!("Set app_container to enabled");
            }
            windows::AppContainerMode::Disabled => (),
        }
        assert_eq!(r.linux.max_open_files, 300);
        assert_eq!(r.linux.secomp_kill, false);
    }
}


pub mod linux {
    pub fn compatible_linux_restrictions() -> LinuxRestrictions {
        LinuxRestrictions {
            max_open_files: 2048,
            secomp_kill: false,
            dev_null_accessible: true,
        }
    }

    pub fn strict_linux_restrictions() -> LinuxRestrictions {
        LinuxRestrictions {
            max_open_files: 2048,
            secomp_kill: false,
            dev_null_accessible: true,
        }
    }

    /// Linux specific restrictions.
    #[derive(Debug, Clone, PartialEq)]
    pub struct LinuxRestrictions {
        /// "rlimit".
        pub max_open_files: u64,

        /// Kill processes on a seccomp violation, rather than just returning an error from the syscall.
        pub secomp_kill: bool,

        /// If the execution closes any of stdin, stdout, or stderr, some programs will
        /// try to open /dev/null to use as a replacement for the closed file descriptor
        /// (Rust's usual startup code will do this).
        /// Because of this behavior, the program needs write access to /dev/null to keep
        /// from triggering a SIGSEGV.  In order to prevent this from happening, the Linux
        /// runtime will grant /dev/null read and write access to the process.
        pub dev_null_accessible: bool,
    }

    /// Create a default AppContainer restriction structure.
    /// This enables the AppContainer, grants no capabilities, and enables desktop isolation.
    pub fn with_max_open_files(
        mut r: super::Restrictions,
        max_open_files: u64,
    ) -> super::Restrictions {
        r.linux.max_open_files = max_open_files;
        r
    }

    pub fn kill_process_on_seccomp_violation(mut r: super::Restrictions) -> super::Restrictions {
        r.linux.secomp_kill = true;
        r
    }
}

pub mod windows {

    /// Create the default Windows restrictions, compatible across upgrades.
    /// Intended for compatibility across version upgrades.  As the library adds new restrictions,
    /// using this invocation should allow the execution of previously working executables to continue to run.
    /// This may mean a less restrictive environment than what the library may allow for, but allows for
    /// upgrading without needing to rerun expensive compatibility testing for every new version.
    pub fn compatible_windows_restrictions(application_name: &String) -> WindowsRestrictions {
        WindowsRestrictions {
            app_container: default_app_container(application_name),
            data_execution_prevention: DataExecutionPreventionMode::ThunkEmulation,
            structured_exception_handler_overwrite_protection: RestrictedAlwaysMode::AlwaysOn,
            aslr: default_aslr_policy(),
            strict_handle_checking: AlwaysMode::AlwaysOn,
            disable_win32k_system_calls: AlwaysMode::Defer, // verified 'Defer' as correct
            disable_extension_points: AlwaysMode::AlwaysOn,
            control_flow_guard: ControlFlowGuardPolicy::Defer, // verified 'Defer' as correct
            dynamic_code: DynamicCodePolicy::AllowOptOut, // verified 'AllowOptOut' as correct
            binary_signature: BinarySignaturePolicy::Defer, // verified 'Defer' as correct
            font_loading_policy: FontLoadingPolicy::AlwaysPrevent,
            image_load_policy: ExecutableImageLoadPolicy {
                no_remote: AlwaysMode::AlwaysOn,
                no_low_label: AlwaysMode::Defer, // verified 'Defer' as correct
                prefer_system32: AlwaysMode::AlwaysOn,
            },
            cet_user_shadow_stack: CETUserShadowStack::AlwaysOn,
            cet_context_ip_validation: CETContextIPValidation::AlwaysOn,
            cet_binary_load_blocking: CETBinaryLoadBlocking::Defer, // verified 'Defer' as correct
            cet_dynamic_apis_out_of_proc_only: AlwaysMode::AlwaysOn,
            restrict_indirect_branch_prediction: true,
            disable_speculative_store_bypass: true,
            disable_fsctl_system_call: AlwaysMode::AlwaysOn,
        }
    }

    /// Create the default Windows restrictions.
    /// This allows for upgrading new versions and taking advantage of newly discovered restrictions.
    /// This has the downside that some executables that used to work may no longer work as expected
    /// after upgrading this library.
    pub fn strict_windows_restrictions(application_name: &String) -> WindowsRestrictions {
        WindowsRestrictions {
            app_container: default_app_container(application_name),
            data_execution_prevention: DataExecutionPreventionMode::ThunkEmulation,
            structured_exception_handler_overwrite_protection: RestrictedAlwaysMode::AlwaysOn,
            aslr: default_aslr_policy(),
            strict_handle_checking: AlwaysMode::AlwaysOn,
            disable_win32k_system_calls: AlwaysMode::Defer, // verified 'Defer' as correct
            disable_extension_points: AlwaysMode::AlwaysOn,
            control_flow_guard: ControlFlowGuardPolicy::Defer, // verified 'Defer' as correct
            dynamic_code: DynamicCodePolicy::AllowOptOut, // verified 'AllowOptOut' as correct
            binary_signature: BinarySignaturePolicy::Defer, // verified 'Defer' as correct
            font_loading_policy: FontLoadingPolicy::AlwaysPrevent,
            image_load_policy: ExecutableImageLoadPolicy {
                no_remote: AlwaysMode::AlwaysOn,
                no_low_label: AlwaysMode::Defer, // verified 'Defer' as correct
                prefer_system32: AlwaysMode::AlwaysOn,
            },
            cet_user_shadow_stack: CETUserShadowStack::AlwaysOn,
            cet_context_ip_validation: CETContextIPValidation::AlwaysOn,
            cet_binary_load_blocking: CETBinaryLoadBlocking::Defer, // verified 'Defer' as correct
            cet_dynamic_apis_out_of_proc_only: AlwaysMode::AlwaysOn,
            restrict_indirect_branch_prediction: true,
            disable_speculative_store_bypass: true,
            disable_fsctl_system_call: AlwaysMode::AlwaysOn,
        }
    }

    /// Windows specific restrictions.
    /// This doesn't cover all settings Windows makes available, but instead just ones that enable
    /// enhanced restrictions that may be too restrictive for most applications.
    #[derive(Debug, Clone, PartialEq)]
    pub struct WindowsRestrictions {
        /// Creates an AppContainer for the runtime,
        /// which is a sandboxing mechanism that restricts the runtime's
        /// access to system resources and user data.
        /// If the application dies before cleanup can happen, this will remain in the user's
        /// operating system.
        /// If not given, this defaults to `true` (enabled).
        // Minimum OS: Windows Vista / Windows Server 2008
        pub app_container: AppContainerMode,

        // ================================================================
        // Windows Process Thread Restrictions.
        // https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute#remarks

        // ----------------------------------------------------------------
        // Minimum OS: Windows Vista / Windows Server 2008
        /// Prevents code from being run from data pages such as the default heap, stacks, and memory pools.
        /// See https://learn.microsoft.com/en-us/windows/win32/memory/data-execution-prevention
        /// This will interfere with some JIT compilers, such as V8, which require executable memory.
        /// If not given, this defaults to `true` (enabled).
        pub data_execution_prevention: DataExecutionPreventionMode,

        /// Windows only; prevents the runtime from overwriting the Structured Exception Handler (SEH).
        /// Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_SEHOP_ENABLE
        pub structured_exception_handler_overwrite_protection: RestrictedAlwaysMode,

        // ----------------------------------------------------------------
        // Minimum OS: Windows 8 / Windows Server 2012
        /// Force an Address Space Layout Randomization (ASLR) policy.
        /// Defaults to AlwaysOn.
        pub aslr: ASLRPolicy,

        /// Strict handle checking causes an exception to be raised
        /// immediately on a bad handle reference. If this policy is not
        /// enabled, a failure status will be returned from the handle
        /// reference instead.  Enabling this allows for preventing a malicious runtime
        /// from handle discovery, but some legitimate operations may also be blocked.
        /// Defaults to `AlwaysOn`.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_STRICT_HANDLE_CHECKS_ALWAYS_OFF
        pub strict_handle_checking: AlwaysMode,

        /// Disabling the win32k system calls prevents the runtime from using the user32, gdi32, and win32k.sys
        /// components, which are responsible for the Windows graphical user interface (GUI) and input handling.
        /// This is a powerful restriction that can prevent a wide range of attacks, but it also means that the
        /// runtime is prevented from common things such as stdin and stdout.
        /// Defaults to `false` (disabled).  While this is a less secure default, writing software compatible
        /// with this requires deliberate effort.
        /// Also note: some Windows auto-run hooks like virus scanners can trip this for executables that do not
        /// have registration as explicitly avoid; for these scenarios, you have very little options around how
        /// to set this to always on.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_OFF
        pub disable_win32k_system_calls: AlwaysMode,

        /// Disabling extension points prevents built-in extension points in AppInit DLLs, Winsock Layered Service Providers (LSPs),
        /// Global Windows Hooks, and Legacy Input Method Editors (IMEs).  Local hooks will still work.
        /// Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_OFF
        pub disable_extension_points: AlwaysMode,

        /// Force the Control Flow Guard (CFG) security feature in a specific mode.
        /// Default is "defer", because this only works when the executable was explicitly compiled to have
        /// CFG enabled.
        /// See https://learn.microsoft.com/en-us/windows/win32/secbp/control-flow-guard
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_OFF
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_EXPORT_SUPPRESSION
        pub control_flow_guard: ControlFlowGuardPolicy,

        /// Enforcement around allowing the program to generate or modify executable code.
        /// If the executable uses Just In Time (JIT), such as with the V8 engine, then this must be allowed use dynamic code.
        pub dynamic_code: DynamicCodePolicy,

        /// Restrictions around the binary executable's signature.
        pub binary_signature: BinarySignaturePolicy,

        /// Restrictions for loading custom fonts.
        pub font_loading_policy: FontLoadingPolicy,

        /// Policies around loading executable images (such as DLLs).
        pub image_load_policy: ExecutableImageLoadPolicy,

        // ----------------------------------------------------------------
        // Minimum OS: Windows 10, version 1709
        /// Protect against sibling hardware threads (hyperthreads) from interfering with indirect branch predictions.
        /// Example: CVE-2017-5715
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_RESTRICT_INDIRECT_BRANCH_PREDICTION_ALWAYS_ON
        pub restrict_indirect_branch_prediction: bool,

        // ----------------------------------------------------------------
        // Minimum OS: Windows 10, version 1809
        /// Disable the Speculative Store Bypass (SSB) feature of CPUs that may be vulnerable to speculative
        /// execution side channel attacks involving SSB (CVE-2018-3639).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_SPECULATIVE_STORE_BYPASS_DISABLE_ALWAYS_ON
        pub disable_speculative_store_bypass: bool,

        // ----------------------------------------------------------------
        // Minimum OS: Windows 10, version 2004
        pub cet_user_shadow_stack: CETUserShadowStack,

        pub cet_context_ip_validation: CETContextIPValidation,

        pub cet_binary_load_blocking: CETBinaryLoadBlocking,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_DYNAMIC_APIS_OUT_OF_PROC_ONLY_ALWAYS_OFF
        pub cet_dynamic_apis_out_of_proc_only: AlwaysMode,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_FSCTL_SYSTEM_CALL_DISABLE_ALWAYS_OFF
        pub disable_fsctl_system_call: AlwaysMode,
    }

    /// Windows AppContainer capabilities.
    #[derive(Debug, Clone, PartialEq)]
    pub enum AppContainerCapability {
        /// Access the user's webcam.
        Webcam,
        /// Access the user's microphone.
        Microphone,
        /// Access the user's location.
        Location,
        /// Access the user's internet connection.
        InternetClient,
        /// Access the user's internet connection, and act as a server.
        InternetClientServer,
        /// Access the user's private network, such as a home or work network.
        PrivateNetworkClientServer,
    }

    /// Windows AppContainer settings.
    #[derive(Debug, Clone, PartialEq)]
    pub struct AppContainer {
        /// The name of the AppContainer to create.  This must be unique across the system, and should be sufficiently random to avoid collisions with other AppContainers.
        pub name: String,

        /// The capabilities to add to the AppContainer.  By default, no capabilities are added.
        pub capabilities: Vec<AppContainerCapability>,

        /// If true, the AppContainer will be created with the "Desktop Isolation" capability, which prevents any
        /// UI elements from the spawned program from interacting with the user's desktop.  This includes an isolated
        /// clipboard, and no windows shown to the user.
        /// TODO it's possible to create a desktop isolate within the user's default AppContainer.  This would
        /// prevent the buildup of cruft that the user would need to deal with related to additional AppContainer objects on
        /// the user's system.
        pub desktop_isolation: bool,

        /// If true, the jail will reuse any existing AppContainer with the given name.
        /// If false, the jail will try to create a new AppContainer with the given name as a prefix.
        /// This will allow multiple processes to share the same AppContainer, and also avoid creating a new one on every execution.
        /// It has the downside that the AppContainer will persist even after the process exits, and that the
        /// processes will share a temporary directory, which could be a security risk if the AppContainer is not properly
        /// configured with capabilities and ACLs.
        /// While there's the opportunity for an attacker to create an AppContainer with the same name, the restrictions for the
        /// app container are handled not by the creation of the app container, but by the capabilities assigned at usage.
        ///
        /// Defaults to true.
        pub reuse_existing: bool,
    }

    /// Windows AppContainer restriction modes.
    #[derive(Debug, Clone, PartialEq)]
    pub enum AppContainerMode {
        /// Creates an AppContainer for the runtime, which is a sandboxing mechanism that restricts the runtime's access to system resources and user data.
        /// If the application dies before cleanup can happen, this will remain in the user's operating system.
        Enabled(AppContainer),

        /// Do not create an AppContainer for the runtime.  This means the runtime will have access to all system resources and user data that the user has access to.
        Disabled,
    }

    /// Create a default AppContainer restriction structure.
    /// This enables the AppContainer, grants no capabilities, and enables desktop isolation.
    pub fn default_app_container(application_name: &String) -> AppContainerMode {
        AppContainerMode::Enabled(AppContainer {
            name: application_name.clone(),
            capabilities: Vec::new(),
            desktop_isolation: true,
            reuse_existing: true,
        })
    }

    pub fn disable_app_container(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.app_container = AppContainerMode::Disabled;
        r
    }

    pub fn disable_desktop_isolation(mut r: super::Restrictions) -> super::Restrictions {
        if let AppContainerMode::Enabled(app_container) = &mut r.windows.app_container {
            app_container.desktop_isolation = false;
        }
        r
    }

    /// Adds a capability to the AppContainer, which grants the runtime access to specific system resources or user data.
    pub fn with_app_container_capability(
        mut r: super::Restrictions,
        capability: AppContainerCapability,
    ) -> super::Restrictions {
        if let AppContainerMode::Enabled(app_container) = &mut r.windows.app_container {
            app_container.capabilities.push(capability);
        }
        r
    }

    /// Force a new AppContainer creation if one with the given name already exists.
    pub fn force_new_app_container(mut r: super::Restrictions) -> super::Restrictions {
        if let AppContainerMode::Enabled(app_container) = &mut r.windows.app_container {
            app_container.reuse_existing = false;
        }
        r
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum DataExecutionPreventionMode {
        /// Do not prevent code from being run from data pages such as the default heap, stacks, and memory pools.
        Disabled,

        /// Enable DEP, but without ATL thunk emulation.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE
        Enabled,

        /// Enable DEP + ATL thunk emulation.
        /// The thunk emulation causes the system to intercept NX faults that originate from the Active Template Library (ATL) thunk layer.
        /// This is the default, because it is the most compatible with existing software.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_DEP_ENABLE
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_DEP_ATL_THUNK_ENABLE
        ThunkEmulation,
    }

    pub fn disable_data_execution_prevention(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.data_execution_prevention = DataExecutionPreventionMode::Disabled;
        r
    }

    pub fn enable_data_execution_prevention(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.data_execution_prevention = DataExecutionPreventionMode::Enabled;
        r
    }

    pub fn defer_structured_exception_handler_overwrite_protection(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        r.windows.structured_exception_handler_overwrite_protection = RestrictedAlwaysMode::Defer;
        r
    }

    /// Windows Address Space Layout Randomization (ASLR) policy.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ASLRPolicy {
        /// Forcibly rebases images that are not dynamic base compatible by acting as though an image base collision happened at load time.
        /// Without this, the executable's ASLR setting is used.  Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_ALWAYS_OFF
        pub force_enabled: AlwaysMode,

        /// The heap terminate on corruption policy causes the heap
        /// to terminate if the heap becomes corrupt.  Note that 'false'
        /// means use the opt-in for the binary, while 'true' forces it on.
        /// Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_HEAP_TERMINATE_ALWAYS_OFF
        pub heap_terminate_on_corruption: AlwaysMode,

        /// Forces relocation, and does not load images that do not have a base relocation section.
        /// Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FORCE_RELOCATE_IMAGES_REQUIRE_RELOCS
        pub require_relocation: bool,

        /// The bottom-up randomization policy, which includes stack randomization options,
        /// causes a random location to be used as the lowest user address.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BOTTOM_UP_ASLR_ALWAYS_OFF
        pub bottom_up_randomization: AlwaysMode,

        /// Enabled the high-entropy 64-bit address space layout randomization policy,
        /// which allows the system to use a larger randomization range for 64-bit processes.
        /// Defaults to `true` (enabled).
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_HIGH_ENTROPY_ASLR_ALWAYS_OFF
        pub high_entropy_randomization: AlwaysMode,
    }

    fn default_aslr_policy() -> ASLRPolicy {
        ASLRPolicy {
            // TODO discover a sufficiently secure but usable set of restrictions.
            force_enabled: AlwaysMode::AlwaysOn,
            require_relocation: true,
            heap_terminate_on_corruption: AlwaysMode::AlwaysOn,
            bottom_up_randomization: AlwaysMode::AlwaysOn,
            high_entropy_randomization: AlwaysMode::AlwaysOn,
        }
    }

    pub fn defer_aslr_policy_forced(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.force_enabled = AlwaysMode::Defer;
        r
    }

    pub fn disable_aslr_policy_forced(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.force_enabled = AlwaysMode::AlwaysOff;
        r
    }

    pub fn defer_aslr_relocation(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.require_relocation = false;
        r
    }

    pub fn defer_aslr_heap_terminate_on_corruption(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        r.windows.aslr.heap_terminate_on_corruption = AlwaysMode::Defer;
        r
    }

    pub fn disable_aslr_heap_terminate_on_corruption(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        r.windows.aslr.heap_terminate_on_corruption = AlwaysMode::AlwaysOff;
        r
    }

    pub fn defer_aslr_bottom_up_randomization(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.bottom_up_randomization = AlwaysMode::Defer;
        r
    }

    pub fn disable_aslr_bottom_up_randomization(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.bottom_up_randomization = AlwaysMode::AlwaysOff;
        r
    }

    pub fn defer_aslr_high_entropy_randomization(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        r.windows.aslr.high_entropy_randomization = AlwaysMode::Defer;
        r
    }

    pub fn disable_aslr_high_entropy_randomization(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        r.windows.aslr.high_entropy_randomization = AlwaysMode::AlwaysOff;
        r
    }

    pub fn prevent_win32k_system_calls(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.disable_win32k_system_calls = AlwaysMode::AlwaysOff;
        r
    }

    /// Ref: PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_DEFER
    pub fn defer_extension_points(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.disable_extension_points = AlwaysMode::Defer;
        r
    }

    /// Note: this API call name looks odd (a kind of double negative) for consistency with the rest of the call names.
    /// Ref: PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_OFF
    pub fn disable_disabled_extension_points(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.disable_extension_points = AlwaysMode::AlwaysOff;
        r
    }

    /// Alias for 'disable_disabled_extension_points'.
    /// Ref: PROCESS_CREATION_MITIGATION_POLICY_EXTENSION_POINT_DISABLE_ALWAYS_OFF
    pub fn allow_extension_points(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.disable_extension_points = AlwaysMode::AlwaysOff;
        r
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum ControlFlowGuardPolicy {
        /// Defer to the binary to determine whether Control Flow Guard (CFG) is enabled for the runtime.
        Defer,

        /// Enables the Control Flow Guard (CFG) settings for the child.
        /// If the binaries have CFG enabled, this will use those settings.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_ALWAYS_ON
        Enable(ControlFlowGuardSettings),
        // The other mode is 'AlwaysOff', but that disables CFG entirely, regardless of what the executable's binary declares.
        // So that mode is not allowed.
    }

    /// Specific settings for the Control Flow Guard.
    /// By default, all these settings defer to the operating system for default
    /// values.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ControlFlowGuardSettings {
        /// This both enables indirect call CFG, and requires loaded EXEs and DLLs were
        /// built with the CFG headers.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_DEFER
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_OFF
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_STRICT_CONTROL_FLOW_GUARD_ALWAYS_ON
        pub require_cfg_images: AlwaysMode,

        /// Requires loaded DLLs to have all exported functions
        /// declared only as dynamic resolution, rather than a static location.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_CONTROL_FLOW_GUARD_EXPORT_SUPPRESSION
        pub export_suppression: bool,
    }

    /// Enable indirect control flow guard with deferred EXE and DLL requirements.
    pub fn indirect_control_flow_guard(mut r: super::Restrictions) -> super::Restrictions {
        match &mut r.windows.control_flow_guard {
            ControlFlowGuardPolicy::Defer => {
                r.windows.control_flow_guard =
                    ControlFlowGuardPolicy::Enable(ControlFlowGuardSettings {
                        require_cfg_images: AlwaysMode::Defer,
                        export_suppression: false,
                    });
            }
            ControlFlowGuardPolicy::Enable(p) => {
                p.require_cfg_images = AlwaysMode::Defer;
            }
        }
        r
    }

    /// Require enabling Control Flow Guard, and require all DLLs and EXEs loaded to have it enabled.
    /// Does not alter the export suppression if the CFG was already enabled.
    pub fn require_control_flow_guard(mut r: super::Restrictions) -> super::Restrictions {
        match &mut r.windows.control_flow_guard {
            ControlFlowGuardPolicy::Defer => {
                r.windows.control_flow_guard =
                    ControlFlowGuardPolicy::Enable(ControlFlowGuardSettings {
                        require_cfg_images: AlwaysMode::AlwaysOn,
                        export_suppression: false,
                    });
            }
            ControlFlowGuardPolicy::Enable(p) => {
                p.require_cfg_images = AlwaysMode::AlwaysOn;
            }
        }
        r
    }

    /// Require enabling Control Flow Guard, and that all loaded DLLs use
    /// dynamic exported functions.  Does not alter the CFG image requirements
    /// if CFG was already enabled.
    pub fn control_flow_guard_export_suppression(
        mut r: super::Restrictions,
    ) -> super::Restrictions {
        match &mut r.windows.control_flow_guard {
            ControlFlowGuardPolicy::Defer => {
                r.windows.control_flow_guard =
                    ControlFlowGuardPolicy::Enable(ControlFlowGuardSettings {
                        require_cfg_images: AlwaysMode::Defer,
                        export_suppression: true,
                    });
            }
            ControlFlowGuardPolicy::Enable(p) => {
                p.export_suppression = true;
            }
        }
        r
    }

    /// Require enabling Control Flow Guard, require all DLLs and EXEs loaded to have it enabled,
    /// and requires loaded DLLs to declare all exported functions with dynamic resolution.
    /// This combines require_control_flow_guard and control_flow_guard_export_suppression.
    pub fn strict_control_flow_guard(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.control_flow_guard = ControlFlowGuardPolicy::Enable(ControlFlowGuardSettings {
            require_cfg_images: AlwaysMode::AlwaysOn,
            export_suppression: true,
        });
        r
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum DynamicCodePolicy {
        /// Do not allow the runtime from generating or modifying executable code at runtime.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON
        AlwaysProhibit,

        /// Let the binary executable decide on the policy to enforce.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_DEFER
        Defer,

        /// Set to "always prohibit" unless the binary explicitly marks it as allowed to modify executable code.
        /// This is the default, and is required for JIT compilers to work.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_ON_ALLOW_OPT_OUT
        AllowOptOut,

        // Force the executable to allow dynamic code generation.
        // Ref: PROCESS_CREATION_MITIGATION_POLICY_PROHIBIT_DYNAMIC_CODE_ALWAYS_OFF
        AlwaysAllow,
    }

    pub fn defer_dynamic_code(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.dynamic_code = DynamicCodePolicy::Defer;
        r
    }

    pub fn prohibit_dynamic_code(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.dynamic_code = DynamicCodePolicy::AlwaysProhibit;
        r
    }

    /// Restrictions on the author of the binary executable.  This can enforce that only Microsoft-signed
    /// binaries can be loaded.
    #[derive(Debug, Clone, PartialEq)]
    pub enum BinarySignaturePolicy {
        /// Defer to the operating system's requirements.
        /// The default.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_DEFER
        Defer,

        /// Only allow binaries that are signed by Microsoft to be loaded into the runtime.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_ON
        AllowOnlyMicrosoft,

        /// While reduces restrictions, in many cases the executable is not a Microsoft binary.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALWAYS_OFF
        AllowAny,

        /// (Not sure - allows anything installed from the Microsoft Store?)
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_BLOCK_NON_MICROSOFT_BINARIES_ALLOW_STORE
        AllowStore,
    }

    // TODO add remaining Restrictions setting functions.

    /// The font loading prevention policy for the process determines whether non-system fonts can be
    /// loaded for a process.
    #[derive(Debug, Clone, PartialEq)]
    pub enum FontLoadingPolicy {
        /// Defer to the operating system's requirements.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_DEFER
        Defer,

        /// Always disable custom font loading.
        /// The default.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_ON
        AlwaysPrevent,

        /// Always allow custom font loading.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_FONT_DISABLE_ALWAYS_OFF
        AlwaysAllow,

        /// Require an audit of non-system fonts.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_AUDIT_NONSYSTEM_FONTS
        AuditNonSystemFonts,
    }

    /// Policies around loading "images" (DLLs, etc) in the restricted process.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ExecutableImageLoadPolicy {
        /// Allow the process to load images (DLLs, etc) from a remote device, such as a UNC share.

        /// Access to images stored on remote devices, such as UNC shares.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_DEFER
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_REMOTE_ALWAYS_OFF
        pub no_remote: AlwaysMode,

        /// Ability to load images marked with "low mandatory" label, as part of the Mandatory Integrity Control (MIC) system in Windows.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_DEFER
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_NO_LOW_LABEL_ALWAYS_OFF
        pub no_low_label: AlwaysMode,

        /// Controls whether the process prefers to load images (DLLs etc) from the System32 subfolder of
        /// the folder in which Windows is installed, rather than from the application directory in the
        /// standard DLL search order.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_DEFER
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_ON
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32_ALWAYS_OFF
        pub prefer_system32: AlwaysMode,
    }

    /// Hardware-enforced Stack Protection (HSP) is a hardware-based security feature where the
    /// CPU verifies function return addresses at runtime by employing a shadow stack mechanism.
    #[derive(Debug, Clone, PartialEq)]
    pub enum CETUserShadowStack {
        /// Only shadow stack violations occurring in modules that are considered compatible with shadow stacks (CETCOMPAT) are fatal.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_DEFER
        Defer,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_ON
        AlwaysOn,

        /// Never cause fatal issues with user shadow stacks.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_ALWAYS_OFF
        AlwaysOff,

        /// All shadow stack violations are fatal.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_CET_USER_SHADOW_STACKS_STRICT_MODE
        StrictMode,
    }

    /// User-mode Hardware-enforced Instruction Pointer validation.
    #[derive(Debug, Clone, PartialEq)]
    pub enum CETContextIPValidation {
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_DEFER
        Defer,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_ON
        AlwaysOn,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_ALWAYS_OFF
        AlwaysOff,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_USER_CET_SET_CONTEXT_IP_VALIDATION_RELAXED_MODE
        RelaxedMode,
    }

    /// Block the load of non-CETCOMPAT/non-EHCONT binaries.
    /// Enabling this requires the loaded binaries to have special flags set during compilation,
    /// so you cannot use this on just any executable.
    #[derive(Debug, Clone, PartialEq)]
    pub enum CETBinaryLoadBlocking {
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_DEFER
        Defer,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_ON
        AlwaysOn,

        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_ALWAYS_OFF
        AlwaysOff,

        /// Allow non-CETCOMPAT binaries, but not non-EHCONT binaries.
        /// Ref: PROCESS_CREATION_MITIGATION_POLICY2_BLOCK_NON_CET_BINARIES_NON_EHCONT
        BlockNonEHCont,
    }

    /// Standard way of forcing a policy to be always on or off, regardless of the executable's choice,
    /// or defer to the binary's choice or OS, if the binary doesn't specify a choice.
    #[derive(Debug, Clone, PartialEq)]
    pub enum AlwaysMode {
        Defer,

        AlwaysOn,

        AlwaysOff,
    }

    /// An intentially restrictive version of the AlwaysMode for particularly sensitive settings.
    /// It prohibits the 'AlwaysOff' option.
    #[derive(Debug, Clone, PartialEq)]
    pub enum RestrictedAlwaysMode {
        Defer,

        AlwaysOn,
    }
}
