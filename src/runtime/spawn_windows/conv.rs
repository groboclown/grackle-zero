// SPDX-License-Identifier: MIT

//! Various type conversion routines.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Convert the OS string into a null-terminated wide (16-bit) C string.
pub fn as_c_str_w(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}
