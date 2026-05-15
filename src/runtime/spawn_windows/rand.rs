// SPDX-License-Identifier: MIT

use windows::Win32::Security::Cryptography;

use crate::runtime::spawn_windows::error::WindowsSandboxError;

const ENCODING: &[u8; 64] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_";

/// Generate 16 characters of random alphanumeric (0-9, a-z, A-Z, _, -) values.
pub fn random_hex_str() -> Result<String, WindowsSandboxError> {
    let mut bytes = [0u8; 16];
    let status = unsafe {
        Cryptography::BCryptGenRandom(
            None, // use a default algorithm
            &mut bytes,
            Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status.0 < 0 {
        return Err(WindowsSandboxError::setup_message(&format!(
            "failed to generate secure random data (NTSTATUS=0x{:08x})",
            status.0 as u32
        )));
    }
    let mut suffix = String::with_capacity(16);
    for b in bytes {
        // Only use 6 bits of the random data.
        suffix.push(ENCODING[(b & 0x3f) as usize] as char);
    }
    Ok(suffix)
}

/// Generate a name, using a prefix str, along with random alphanumeric characters + '_' and '-'.
pub fn random_str_name<'a>(prefix: &'a str) -> Result<String, WindowsSandboxError> {
    Ok(format!("{}-{}", prefix, random_hex_str()?))
}

/// Generate a name, using a prefix String, along with random alphanumeric characters + '_' and '-'.
pub fn random_string_name<'a>(prefix: &'a String) -> Result<String, WindowsSandboxError> {
    Ok(format!("{}-{}", prefix, random_hex_str()?))
}
