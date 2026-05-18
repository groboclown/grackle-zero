// SPDX-License-Identifier: MIT

//! Various type conversion routines.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};

/// Convert the OS string into a null-terminated wide (16-bit) C string.
pub fn as_c_str_w(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// Convert the Rust-allocated wide-character array into a Rust String.
/// Because it was allocated by Rust with a known length, this has no chance of buffer overrun.
pub fn c_str_w_as_str(s: &[u16]) -> String {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..len])
}

/// Convert the Windows native PWSTR into an OsString.
/// Performs unsafe pointer arithmetic.
pub unsafe fn pwstr_as_osstring(pwstr: windows::core::PWSTR) -> OsString {
    if pwstr.is_null() {
        return OsString::new();
    }

    let mut len = 0usize;
    let p = pwstr.0;

    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }

    let wide: &[u16] = unsafe { std::slice::from_raw_parts(p, len) };
    OsString::from_wide(wide)
}

/// Check if the given Windows error code matches the native WIN32_ERROR value.
pub fn hresult_err_eq(
    err: &windows::core::Error,
    win32_err: windows::Win32::Foundation::WIN32_ERROR,
) -> bool {
    err.code() == win32_err.to_hresult()
}

/// Check if the given Windows error code matches the native WIN32_ERROR value.
pub fn hresult_eq(err: &windows::core::Error, hresult: windows_core::HRESULT) -> bool {
    err.code() == hresult
}
