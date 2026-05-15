// SPDX-License-Identifier: MIT

//! Construct all the kinds of restrictions possible.
//! This does not create the permutations, but instead one for each option value,
//! then some known, tricky combinations.

use gracklezero::restrictions::*;

pub const APP_NAME: &str = "gracklezero-integration-tests";

#[allow(unused)]
pub fn generate_restrictions<'a>() -> Vec<(String, Restrictions)> {
    let base_values: &[(&str, fn(Restrictions) -> Restrictions)] = &[
        // Start least restrictive and build up to most restrictive.
        ("base", |a| a),
        (
            "data_execution_prevention-enabled",
            windows::enable_data_execution_prevention,
        ),
        (
            "data_execution_prevention-thunk",
            with_thunk_data_execution_prevention,
        ),
        (
            "structured_exception_handler_overwrite_protection-thunk",
            with_structured_exception_handler_overwrite_protection,
        ),
        ("aslr-force_enabled", with_aslr_force_enabled),
        ("aslr-require-relocation", with_aslr_require_relocation),
        (
            "aslr-heap_terminate_on_corruption-on",
            with_aslr_heap_terminate_on_corruption_on,
        ),
        (
            "aslr-heap_terminate_on_corruption-defer",
            windows::defer_aslr_heap_terminate_on_corruption,
        ),
        (
            "aslr-bottom_up_randomization-on",
            with_aslr_bottom_up_randomization_on,
        ),
        (
            "slr-bottom_up_randomization-defer",
            windows::defer_aslr_bottom_up_randomization,
        ),

        // Note: explicitly omit win32k disable, due to issues with
        // native auto-run hooks like virus scanners that can trigger
        // executables to unexpectedly fail.

        // TODO add many other combinatorics.
    ];

    let base = base_restrictions();
    let app = app_container_restrictions();
    let desk = desktop_restrictions();
    let mut ret = Vec::new();
    for b in base_values {
        ret.push((format!("{}", &b.0), b.1(base.clone())));
        ret.push((format!("app-{}", &b.0), b.1(app.clone())));
        ret.push((format!("desk-{}", &b.0), b.1(desk.clone())));
    }
    ret.push(("compat".to_string(), create_compat_restrictions(&APP_NAME.to_string())));
    ret.push(("strict".to_string(), create_strict_restrictions(&APP_NAME.to_string())));

    ret
}

fn with_thunk_data_execution_prevention(mut r: Restrictions) -> Restrictions {
    r.windows.data_execution_prevention = windows::DataExecutionPreventionMode::ThunkEmulation;
    r
}

fn with_structured_exception_handler_overwrite_protection(mut r: Restrictions) -> Restrictions {
    r.windows.structured_exception_handler_overwrite_protection =
        windows::RestrictedAlwaysMode::AlwaysOn;
    r
}

fn with_aslr_force_enabled(mut r: Restrictions) -> Restrictions {
    r.windows.aslr.force_enabled = windows::AlwaysMode::AlwaysOn;
    r
}

fn with_aslr_require_relocation(mut r: Restrictions) -> Restrictions {
    r.windows.aslr.require_relocation = true;
    r
}

fn with_aslr_heap_terminate_on_corruption_on(mut r: Restrictions) -> Restrictions {
    r.windows.aslr.heap_terminate_on_corruption = windows::AlwaysMode::AlwaysOn;
    r
}

fn with_aslr_bottom_up_randomization_on(mut r: Restrictions) -> Restrictions {
    r.windows.aslr.bottom_up_randomization = windows::AlwaysMode::AlwaysOn;
    r
}

pub fn desktop_restrictions() -> Restrictions {
    let mut r = app_container_restrictions();
    if let windows::AppContainerMode::Enabled(ac) = &mut r.windows.app_container {
        ac.desktop_isolation = true;
    }
    r
}

fn app_container_restrictions() -> Restrictions {
    let mut r = base_restrictions();
    r.windows.app_container = windows::AppContainerMode::Enabled(windows::AppContainer {
        name: APP_NAME.to_string(),
        capabilities: Vec::new(),
        desktop_isolation: false,
        reuse_existing: true,
    });
    r
}

/// Create the simplest, most open, restrictions.
fn base_restrictions() -> Restrictions {
    Restrictions {
        linux: linux::LinuxRestrictions {
            max_open_files: 20,
            secomp_kill: false,
        },
        windows: windows::WindowsRestrictions {
            app_container: windows::AppContainerMode::Disabled,
            data_execution_prevention: windows::DataExecutionPreventionMode::Disabled,
            structured_exception_handler_overwrite_protection: windows::RestrictedAlwaysMode::Defer,
            aslr: windows::ASLRPolicy {
                force_enabled: windows::AlwaysMode::AlwaysOff,
                require_relocation: false,
                heap_terminate_on_corruption: windows::AlwaysMode::AlwaysOff,
                bottom_up_randomization: windows::AlwaysMode::AlwaysOff,
                high_entropy_randomization: windows::AlwaysMode::AlwaysOff,
            },
            strict_handle_checking: windows::AlwaysMode::AlwaysOff,
            disable_win32k_system_calls: windows::AlwaysMode::AlwaysOff,
            disable_extension_points: windows::AlwaysMode::AlwaysOff,
            control_flow_guard: windows::ControlFlowGuardPolicy::Defer,
            dynamic_code: windows::DynamicCodePolicy::AlwaysAllow,
            binary_signature: windows::BinarySignaturePolicy::AllowAny,
            font_loading_policy: windows::FontLoadingPolicy::AlwaysAllow,
            image_load_policy: windows::ExecutableImageLoadPolicy {
                no_remote: windows::AlwaysMode::AlwaysOff,
                no_low_label: windows::AlwaysMode::AlwaysOff,
                prefer_system32: windows::AlwaysMode::AlwaysOff,
            },
            restrict_indirect_branch_prediction: false,
            disable_speculative_store_bypass: false,
            cet_user_shadow_stack: windows::CETUserShadowStack::AlwaysOff,
            cet_context_ip_validation: windows::CETContextIPValidation::AlwaysOff,
            cet_binary_load_blocking: windows::CETBinaryLoadBlocking::AlwaysOff,
            cet_dynamic_apis_out_of_proc_only: windows::AlwaysMode::AlwaysOff,
            disable_fsctl_system_call: windows::AlwaysMode::AlwaysOff,
        },
    }
}
