// SPDX-License-Identifier: MIT

/// Explicit list of restrictions to apply to the spawned runtime.
/// By default, the system enables all of them.  They must be explicitly disabled if not wanted.
/// Some restrictions apply to a specific operating system or runtime environment.
/// Some restrictions require another restriction in order to run.
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
    use crate::{compat_restrictions, strict_restrictions};
    use super::{linux, windows};

    #[test]
    fn test_restrictions() {
        let r = strict_restrictions!("test_app",
            |r| { linux::set_max_open_files(r, 4096) },
            linux::kill_process_on_seccomp_violation,
            windows::disable_app_container,
            windows::disable_desktop_isolation,
            windows::disable_data_execution_prevention,
            |r| { windows::add_app_container_capability(r, windows::AppContainerCapability::Webcam) },
            windows::defer_aslr_policy,
        );
        assert_eq!(r.linux.max_open_files, 4096);
        assert_eq!(r.linux.secomp_kill, true);

        let r = compat_restrictions!("test_app",
            windows::disable_app_container,
            windows::disable_desktop_isolation,
            windows::disable_data_execution_prevention,
            |r| { windows::add_app_container_capability(r, windows::AppContainerCapability::Webcam) },
            windows::defer_aslr_policy,
        );
        assert_eq!(r.linux.max_open_files, 2048);
        assert_eq!(r.linux.secomp_kill, false);
    }
}


pub mod linux {

    pub fn compatible_linux_restrictions() -> LinuxRestrictions {
        LinuxRestrictions {
            max_open_files: 2048,
            secomp_kill: false,
        }
    }

    pub fn strict_linux_restrictions() -> LinuxRestrictions {
        LinuxRestrictions {
            max_open_files: 2048,
            secomp_kill: false,
        }
    }


    /// Linux specific restrictions.
    pub struct LinuxRestrictions {
        pub max_open_files: u64,

        /// Kill processes on a seccomp violation, rather than just returning an error from the syscall.
        pub secomp_kill: bool,
    }

    /// Create a default AppContainer restriction structure.
    /// This enables the AppContainer, grants no capabilities, and enables desktop isolation.
    pub fn set_max_open_files(mut r: super::Restrictions, max_open_files: u64) -> super::Restrictions {
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
            data_execution_prevention: default_data_execution_prevention(),
            structured_exception_handler_overwrite_protection: true,
            aslr: default_aslr_policy(),
            heap_terminate_on_corruption: true,
            strict_handle_checking: true,
            disable_win32k_system_calls: false,
            disable_extension_points: true,
            control_flow_guard: ControlFlowGuardPolicy::AlwaysOn(true),
            dynamic_code: DynamicCodePolicy::AllowOptOut,
            binary_signature: BinarySignaturePolicy::Defer,
            font_loading_policy: FontLoadingPolicy::AlwaysPrevent,
            image_load_policy: ExecutableImageLoadPolicy {
                no_remote: ImageLoadPolicy::AlwaysOn,
                no_low_label: ImageLoadPolicy::Defer,
                prefer_system32: ImageLoadPolicy::AlwaysOn,
            },
            cet_user_shadow_stack: CETUserShadowStack::AlwaysOn,
            cet_context_ip_validation: CETContextIPValidation::AlwaysOn,
            cet_binary_load_blocking: CETBinaryLoadBlocking::AlwaysOn,
            cet_dynamic_apis_out_of_proc_only: CETDynamicApisOutOfProcOnly::AlwaysOn,
            restrict_indirect_branch_prediction: true,
            disable_speculative_store_bypass: true,
            disable_fsctl_system_call: FSCTLSystemCallDisablePolicy::AlwaysOn,
        }
    }

    /// Create the default Windows restrictions.
    /// This allows for upgrading new versions and taking advantage of newly discovered restrictions.
    /// This has the downside that some executables that used to work may no longer work as expected
    /// after upgrading this library.
    pub fn strict_windows_restrictions(application_name: &String) -> WindowsRestrictions {
        WindowsRestrictions {
            app_container: default_app_container(application_name),
            data_execution_prevention: default_data_execution_prevention(),
            structured_exception_handler_overwrite_protection: true,
            aslr: default_aslr_policy(),
            heap_terminate_on_corruption: true,
            strict_handle_checking: true,
            disable_win32k_system_calls: false,
            disable_extension_points: true,
            control_flow_guard: ControlFlowGuardPolicy::AlwaysOn(true),
            dynamic_code: DynamicCodePolicy::AllowOptOut,
            binary_signature: BinarySignaturePolicy::Defer,
            font_loading_policy: FontLoadingPolicy::AlwaysPrevent,
            image_load_policy: ExecutableImageLoadPolicy {
                no_remote: ImageLoadPolicy::AlwaysOn,
                no_low_label: ImageLoadPolicy::Defer,
                prefer_system32: ImageLoadPolicy::AlwaysOn,
            },
            cet_user_shadow_stack: CETUserShadowStack::AlwaysOn,
            cet_context_ip_validation: CETContextIPValidation::AlwaysOn,
            cet_binary_load_blocking: CETBinaryLoadBlocking::AlwaysOn,
            cet_dynamic_apis_out_of_proc_only: CETDynamicApisOutOfProcOnly::AlwaysOn,
            restrict_indirect_branch_prediction: true,
            disable_speculative_store_bypass: true,
            disable_fsctl_system_call: FSCTLSystemCallDisablePolicy::AlwaysOn,
        }
    }


    /// Windows specific restrictions.
    /// This doesn't cover all settings Windows makes available, but instead just ones that enable 
    /// enhanced restrictions that may be too restrictive for most applications.
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
        pub structured_exception_handler_overwrite_protection: bool,


        // ----------------------------------------------------------------
        // Minimum OS: Windows 8 / Windows Server 2012

        /// Force an Address Space Layout Randomization (ASLR) policy.
        /// Defaults to AlwaysOn.
        pub aslr: ASLRPolicy,

        /// The heap terminate on corruption policy causes the heap
        /// to terminate if the heap becomes corrupt.  Note that 'false'
        /// means use the opt-in for the binary, while 'true' forces it on.
        /// Defaults to `true` (enabled).
        pub heap_terminate_on_corruption: bool,

        /// Strict handle checking causes an exception to be raised
        /// immediately on a bad handle reference. If this policy is not
        /// enabled, a failure status will be returned from the handle
        /// reference instead.  Enabling this allows for preventing a malicious runtime
        /// from handle discovery, but some legitimate operations may also be blocked.
        /// Defaults to `true` (enabled).
        pub strict_handle_checking: bool,

        /// Disabling the win32k system calls prevents the runtime from using the user32, gdi32, and win32k.sys
        /// components, which are responsible for the Windows graphical user interface (GUI) and input handling.
        /// This is a powerful restriction that can prevent a wide range of attacks, but it also means that the
        /// runtime is prevented from common things such as stdin and stdout.
        /// Defaults to `false` (disabled).  While this is a less secure default, writing software compatible
        /// with this requires deliberate effort.
        pub disable_win32k_system_calls: bool,

        /// Disabling extension points prevents built-in extension points in AppInit DLLs, Winsock Layered Service Providers (LSPs),
        /// Global Windows Hooks, and Legacy Input Method Editors (IMEs).  Local hooks will still work.
        /// Defaults to `true` (enabled).
        pub disable_extension_points: bool,

        /// Force the Control Flow Guard (CFG) security feature in a specific mode.
        /// See https://learn.microsoft.com/en-us/windows/win32/secbp/control-flow-guard
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
        pub restrict_indirect_branch_prediction: bool,


        // ----------------------------------------------------------------
        // Minimum OS: Windows 10, version 1809

        /// Disable the Speculative Store Bypass (SSB) feature of CPUs that may be vulnerable to speculative
        /// execution side channel attacks involving SSB (CVE-2018-3639).
        pub disable_speculative_store_bypass: bool,

        // ----------------------------------------------------------------
        // Minimum OS: Windows 10, version 2004

        pub cet_user_shadow_stack: CETUserShadowStack,

        pub cet_context_ip_validation: CETContextIPValidation,

        pub cet_binary_load_blocking: CETBinaryLoadBlocking,

        pub cet_dynamic_apis_out_of_proc_only: CETDynamicApisOutOfProcOnly,

        pub disable_fsctl_system_call: FSCTLSystemCallDisablePolicy,
    }


    /// Windows AppContainer capabilities.
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
    pub struct AppContainer {
        /// The name of the AppContainer to create.  This must be unique across the system, and should be sufficiently random to avoid collisions with other AppContainers.
        /// If not given, this defaults to a random name.
        pub name: String,

        /// The capabilities to add to the AppContainer.  By default, no capabilities are added.
        pub capabilities: Vec<AppContainerCapability>,

        /// If true, the AppContainer will be created with the "Desktop Isolation" capability, which prevents any
        /// UI elements from the spawned program from interacting with the user's desktop.  This includes an isolated
        /// clipboard, and no windows shown to the user.
        pub desktop_isolation: bool,
    }


    /// Windows AppContainer restriction modes.
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
    pub fn add_app_container_capability(mut r: super::Restrictions, capability: AppContainerCapability) -> super::Restrictions {
        if let AppContainerMode::Enabled(app_container) = &mut r.windows.app_container {
            app_container.capabilities.push(capability);
        }
        r
    }


    pub enum DataExecutionPreventionMode {
        /// Do not prevent code from being run from data pages such as the default heap, stacks, and memory pools.
        Disabled,

        /// Enable DEP, but without ATL thunk emulation.
        Enabled,

        /// Enable DEP + ATL thunk emulation.
        /// The thunk emulation causes the system to intercept NX faults that originate from the Active Template Library (ATL) thunk layer.
        /// This is the default, because it is the most compatible with existing software.
        ThunkEmulation,
    }


    fn default_data_execution_prevention() -> DataExecutionPreventionMode {
        DataExecutionPreventionMode::ThunkEmulation
    }


    pub fn disable_data_execution_prevention(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.data_execution_prevention = DataExecutionPreventionMode::Disabled;
        r
    }

    pub fn enable_data_execution_prevention(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.data_execution_prevention = DataExecutionPreventionMode::Enabled;
        r
    }


    /// Windows Address Space Layout Randomization (ASLR) policy.
    pub struct ASLRPolicy {
        /// Forcibly rebases images that are not dynamic base compatible by acting as though an image base collision happened at load time.
        /// Without this, the executable's ASLR setting is used.  Defaults to `true` (enabled).
        pub force_enabled: bool,

        /// Forces relocation, and does not load images that do not have a base relocation section.
        /// Defaults to `true` (enabled).
        pub require_relocation: bool,

        /// The bottom-up randomization policy, which includes stack randomization options,
        /// causes a random location to be used as the lowest user address. 
        pub bottom_up_randomization: bool,

        /// Enabled the high-entropy 64-bit address space layout randomization policy,
        /// which allows the system to use a larger randomization range for 64-bit processes.
        /// Defaults to `true` (enabled).
        pub high_entropy_randomization: bool,
    }


    fn default_aslr_policy() -> ASLRPolicy {
        ASLRPolicy {
            force_enabled: true,
            require_relocation: true,
            bottom_up_randomization: true,
            high_entropy_randomization: true,
        }
    }

    pub fn defer_aslr_policy(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.force_enabled = false;
        r
    }

    pub fn defer_aslr_relocation(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.require_relocation = false;
        r
    }

    pub fn disable_aslr_bottom_up_randomization(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.bottom_up_randomization = false;
        r
    }

    pub fn disable_aslr_high_entropy_randomization(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.aslr.high_entropy_randomization = false;
        r
    }


    pub enum ControlFlowGuardPolicy {
        /// Defer to the binary to determine whether Control Flow Guard (CFG) is enabled for the runtime.
        Defer,

        /// Force Control Flow Guard (CFG) to be enabled for the runtime, regardless of what the executable's binary declares.
        /// If the value is true, then this will be in 'strict' mode, which means that the runtime will be prevented from loading any
        /// modules that are not CFG compatible.
        /// The default value, set to 'true'.
        AlwaysOn(bool),

        ExportSuppression,

        // The other mode is 'AlwaysOff', but that disables CFG entirely, regardless of what the executable's binary declares.
        // So that mode is not allowed.
    }

    pub fn defer_control_flow_guard(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.control_flow_guard = ControlFlowGuardPolicy::Defer;
        r
    }

    pub fn cfg_std_on(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.control_flow_guard = ControlFlowGuardPolicy::AlwaysOn(false);
        r
    }

    pub fn cfg_export_suppression(mut r: super::Restrictions) -> super::Restrictions {
        r.windows.control_flow_guard = ControlFlowGuardPolicy::ExportSuppression;
        r
    }


    pub enum DynamicCodePolicy {
        /// Do not allow the runtime from generating or modifying executable code at runtime.
        AlwaysProhibit,

        /// Let the binary executable decide on the policy to enforce.
        Defer,

        /// Set to "always prohibit" unless the binary explicitly marks it as allowed to modify executable code.
        /// This is the default, and is required for JIT compilers to work.
        AllowOptOut,
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
    pub enum BinarySignaturePolicy {
        /// Defer to the operating system's requirements.
        /// The default.
        Defer,

        /// Only allow binaries that are signed by Microsoft to be loaded into the runtime.
        AllowOnlyMicrosoft,

        /// While reduces restrictions, in many cases the executable is not a Microsoft binary.
        AllowAny,

        /// (Not sure - allows anything installed from the Microsoft Store?)
        AllowStore,
    }

    // TODO add remaining Restrictions setting functions.


    /// The font loading prevention policy for the process determines whether non-system fonts can be
    /// loaded for a process.
    pub enum FontLoadingPolicy {
        /// Defer to the operating system's requirements.
        Defer,

        /// Always disable custom font loading.
        /// The default.
        AlwaysPrevent,

        /// Always allow custom font loading.
        AlwaysAllow,

        /// Require an audit of non-system fonts.
        AuditNonSystemFonts,
    }

    /// Policies around loading "images" (DLLs, etc) in the restricted process.
    pub struct ExecutableImageLoadPolicy {
        /// Allow the process to load images (DLLs, etc) from a remote device, such as a UNC share.

        /// Access to images stored on remote devices, such as UNC shares.
        no_remote: ImageLoadPolicy,

        /// Ability to load images marked with "low mandatory" label, as part of the Mandatory Integrity Control (MIC) system in Windows.
        no_low_label: ImageLoadPolicy,

        /// Controls whether the process prefers to load images (DLLs etc) from the System32 subfolder of
        /// the folder in which Windows is installed, rather than from the application directory in the
        /// standard DLL search order.
        prefer_system32: ImageLoadPolicy,
    }

    /// General policy description around image loading.
    pub enum ImageLoadPolicy {
        Defer,

        /// Always turn on the policy, regardless of what the executable requests.
        AlwaysOn,

        /// Always disable the policy, regardless of what the executable requests.
        AlwaysOff,
    }

    /// Hardware-enforced Stack Protection (HSP) is a hardware-based security feature where the
    /// CPU verifies function return addresses at runtime by employing a shadow stack mechanism.
    pub enum CETUserShadowStack {
        /// Only shadow stack violations occurring in modules that are considered compatible with shadow stacks (CETCOMPAT) are fatal.
        Defer,

        AlwaysOn,

        /// Never cause fatal issues with user shadow stacks.
        AlwaysOff,

        /// All shadow stack violations are fatal.
        StrictMode,
    }

    /// User-mode Hardware-enforced Instruction Pointer validation.
    pub enum CETContextIPValidation {
        Defer,

        AlwaysOn,

        AlwaysOff,

        RelaxedMode,
    }

    /// Block the load of non-CETCOMPAT/non-EHCONT binaries.
    pub enum CETBinaryLoadBlocking {
        Defer,

        AlwaysOn,

        AlwaysOff,

        /// Allow non-CETCOMPAT binaries, but not non-EHCONT binaries.
        BlockNonEHCont,
    }

    /// Restrict certain HSP APIs used to specify security properties of dynamic code to only be callable from outside of the process.
    pub enum CETDynamicApisOutOfProcOnly {
        Defer,

        AlwaysOn,

        AlwaysOff,
    }

    /// Prevent a process from making NtFsControlFile calls.
    pub enum FSCTLSystemCallDisablePolicy {
        Defer,

        AlwaysOn,

        AlwaysOff,
    }
}
