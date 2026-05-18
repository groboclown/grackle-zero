// SPDX-License-Identifier: MIT

#[macro_export]
#[doc(hidden)]
macro_rules! __call_restriction {
    ( $r:expr $(,)? ) => {
        $r
    };
    ( $r:expr, ( $n:expr $(, $x:expr )* $(,)? ), $($rest:tt)* ) => {
        $crate::__call_restriction!($n($r $(, $x)* ), $($rest)*)
    };
    ( $r:expr, ( $n:expr $(, $x:expr )* $(,)? ) $(,)? ) => {
        $n($r $(, $x)*)
    };
    ( $r:expr, $n:expr, $($rest:tt)* ) => {
        $crate::__call_restriction!($n($r), $($rest)*)
    };
    ( $r:expr, $n:expr $(,)? ) => {
        $n($r)
    };
}

/// Create a default `restrictions::Restrictions` structure.
///
/// This uses options that attempt to keep the restrictions structure compatible across library upgrades;
/// if new versions of the library introduce new capabilities for limiting the execution, then this will
/// use the mode that keeps as close compatibility across *minor* and *patch* versions.
/// The authors give no guarantees for compatibility across major versions.  This gives library
/// authors the ability to restructure the restrictions without needing to put high testing around
/// compatibility; though the authors will make efforts to keep upgrades as easy as possible.
///
/// The macro allows for using the restrictions helper functions to change the default restrictions.
///
/// # Examples
///
/// ## Create just compatible restrictions
///
/// In this example, the program creates the default restriction mode.
///
/// ```
/// use gracklezero;
///
/// let r = gracklezero::compat_restrictions!("application-name");
/// ```
///
/// ## Create simple alterations
///
/// In this example, the program creates the default restrictions, but with a limitation on
/// the kind of program that Windows environments can run.
///
/// ```
/// use gracklezero;
///
/// let r = gracklezero::compat_restrictions!(
///     "application-name",
///     gracklezero::restrictions::windows::prohibit_dynamic_code,
/// );
/// ```
///
/// ## Create alterations that require a value.
///
/// In this example, the program alters the default restrictions to set the maximum number of
/// open files for Linux environments to 4096, and to allow the executable in Windows environments
/// access to the webcam:
///
/// ```
/// use gracklezero;
///
/// let r = gracklezero::compat_restrictions!(
///     "another-application-name",
///     (gracklezero::restrictions::linux::with_max_open_files, 4096),
///     (gracklezero::restrictions::windows::with_app_container_capability, gracklezero::restrictions::windows::AppContainerCapability::Webcam),
/// );
/// ```
///
#[macro_export]
macro_rules! compat_restrictions {
    ( $n:expr ) => {
        {
            $crate::restrictions::create_compat_restrictions(&String::from($n))
        }
    };
    ( $n:expr, $($x:tt)+ ) => {
        {
            $crate::__call_restriction!($crate::restrictions::create_compat_restrictions(&String::from($n)), $($x)+)
        }
    }
}

/// Create a default `restrictions::Restrictions` structure.
///
/// The generated structure will attempt to keep the structure configured for high security.
/// This means that, across minor and patch upgrades, this may introduce new security features that
/// makes what previously would work now no longer work.
///
/// This follows the same usage pattern as the `compat_restrictions!` macro.
#[macro_export]
macro_rules! strict_restrictions {
    ( $n:expr ) => {
        {
            $crate::restrictions::create_strict_restrictions(&String::from($n))
        }
    };
    ( $n:expr, $($x:tt)+ ) => {
        {
            $crate::__call_restriction!($crate::restrictions::create_strict_restrictions(&String::from($n)), $($x)+)
        }
    }
}
