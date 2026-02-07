//! Windows Argument Quoting Rules
//! 
//! Unfortunately, there currently is no crate that supplies this logic, and
//! the Rust standard library has these as crate private.  Therefore, there's
//! no choice (at the moment) than to implement this tricky logic here.
//! 
//! See "Everyone quotes command line arguments the wrong way":
//!   https://learn.microsoft.com/en-us/archive/blogs/twistylittlepassagesallalike/everyone-quotes-command-line-arguments-the-wrong-way
//! and
//!   https://docs.microsoft.com/en-us/archive/blogs/larryosterman/the-windows-command-line-is-just-a-string
//! 
//! Note that this also must include the command name that is traditionally
//! passed as arg 0.

use std::{ffi::{OsStr, OsString}, os::windows::ffi::OsStrExt};

use crate::runtime::error::SandboxError;

/// Turn a hashmap of environment variables into a format usable by launch_restricted.
pub fn encode_env_strings(env: &[(OsString, OsString)]) -> Result<Vec<u16>, SandboxError> {
    let mut block: Vec<u16> = Vec::new();

    for (k, v) in env {
        let k = enforce_no_equal(k)?;
        let v = enforce_no_zero(v)?;
        
        block.extend(k.encode_wide());
        block.push(':' as u16);
        block.extend(v.encode_wide());
        block.push(0); // NUL terminator for this entry
    }

    block.push(0); // extra NUL terminator ends the block
    Ok(block)
}

/// Quote the command and arguments into the argument parameter to the launch function.
pub fn quote_arguments<'a, 'b, 'c>(cmd: &'a OsStr, args: &'b Vec<OsString>) -> Result<Vec<u16>, SandboxError> {
    let mut ret = vec![];
    append_arg(&mut ret, &OsString::from(cmd))?;
    for arg in args {
        ret.push(' ' as u16);
        append_arg(&mut ret, arg)?;
    }
    Ok(ret)
}


fn append_arg<'a, 'b>(cmd: &'a mut Vec<u16>, arg: &'b OsString) -> Result<(), SandboxError> {
    let arg = enforce_no_zero(arg)?;
    if !requires_quoting(arg) {
        let arg: Vec<u16> = arg.encode_wide().collect();
        cmd.extend_from_slice(arg.as_slice());
        return Ok(());
    }

    // Perform quoting.
    cmd.push('"' as u16);
    let mut backslash_count = 0;
    for c in arg.encode_wide() {
        if c == '\\' as u16 {
            backslash_count += 1;
            continue;
        }
        if c == '"' as u16 {
            // Escape all the backslashes, and add one for the escaped '"'.
            for _ in 0..(backslash_count * 2 + 1) {
                cmd.push('\\' as u16);
            }
            cmd.push(c);
        } else {
            // Backslashes aren't special.
            for _ in 0..backslash_count {
                cmd.push('\\' as u16);
            }
            cmd.push(c);
        }
        backslash_count = 0;
    }

    // Escape all the trailing backslashes.
    // Let the final '"' be still considered a meta-character.
    for _ in 0..(backslash_count * 2) {
        cmd.push('\\' as u16);
    }

    cmd.push('"' as u16);
    Ok(())
}


fn enforce_no_zero(val: &OsString) -> Result<&OsStr, SandboxError> {
    let ret = OsStr::new(val);
    if ret.encode_wide().any(|b| b == 0) {
        Err(SandboxError::JailSetup("nul byte found in value".to_string()))
    } else {
        Ok(ret)
    }
}

fn enforce_no_equal(val: &OsString) -> Result<&OsStr, SandboxError> {
    let ret = OsStr::new(val);
    if ret.encode_wide().any(|b| b == '=' as u16 || b == 0) {
        Err(SandboxError::JailSetup("Environment variable key must not contain '=' or nul".to_string()))
    } else {
        Ok(ret)
    }
}

fn requires_quoting(val: &OsStr) -> bool {
    val.is_empty() || 
    val.encode_wide().any(char_requires_quoting)
}

fn char_requires_quoting(b: u16) -> bool {
    b == ' ' as u16
    || b == '\t' as u16
    || b == '\n' as u16
    || b == 0x0bu16  // vertical tab (\v in c)
    || b == '"' as u16
}

#[cfg(test)]
mod tests {
    use super::{encode_env_strings, quote_arguments};
    use crate::runtime::error::SandboxError;
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    fn utf16_to_string(vec: &[u16]) -> String {
        String::from_utf16(vec).expect("valid UTF-16")
    }

    fn join_env_block(pairs: &[&str]) -> Vec<u16> {
        let mut ret = vec![];
        for p in pairs {
            let v: Vec<u16> = OsStr::new(p).encode_wide().collect();
            ret.extend_from_slice(&v);
            ret.push(0);
        }
        ret.push(0);
        ret
    }

    #[test]
    fn encode_env_strings_basic_and_termination() {
        let block = encode_env_strings(&[
            (OsString::from("FOO"), OsString::from("BAR")),
            (OsString::from("BAZ"), OsString::from("QUX")),
        ]).expect("encoding should succeed");

        assert_eq!(block, join_env_block(&["FOO=BAR", "BAZ=QUX"]));
    }

    #[test]
    fn encode_env_strings_error_key_with_equal() {
        let err = encode_env_strings(&[
            (OsString::from("BAD=KEY"), OsString::from("VAL")),
        ]).unwrap_err();
        match err {
            SandboxError::JailSetup(msg) => {
                assert!(msg.contains("must not contain '=' or nul"));
            }
            e => panic!("unexpected error {:?}", e),
        }
    }

    #[test]
    fn encode_env_strings_error_value_with_nul() {
        // Value contains an interior NUL
        let err = encode_env_strings(&[
            (OsString::from("KEY"), OsString::from_wide(&[b'X' as u16, 0, b'Y' as u16])),
        ]).unwrap_err();
        match err {
            SandboxError::JailSetup(msg) => {
                assert!(msg.contains("nul byte"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn encode_env_strings_error_key_with_nul() {
        // Key contains an interior NUL
        let err = encode_env_strings(&[
            (OsString::from_wide(&[b'X' as u16, 0, b'Y' as u16]), OsString::from("VAL")),
        ]).unwrap_err();
        match err {
            SandboxError::JailSetup(msg) => {
                assert!(msg.contains("must not contain '=' or nul"), "unexpected error: {:?}", msg);
            }
            e => panic!("unexpected error variant: {:?}", e),
        }
    }

    #[test]
    fn quote_arguments_no_quoting_needed() {
        let cmd = OsStr::new("prog.exe");
        let args = vec![OsString::from("foo"), OsString::from("bar")];
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);
        assert_eq!(s, "prog.exe foo bar");
    }

    #[test]
    fn quote_arguments_with_space() {
        let cmd = OsStr::new("prog.exe");
        let args = vec![OsString::from("with space")];
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);
        assert_eq!(s, "prog.exe \"with space\"");
    }

    #[test]
    fn quote_arguments_empty_arg() {
        let cmd = OsStr::new("prog.exe");
        let args = vec![OsString::from("")];
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);
        assert_eq!(s, "prog.exe \"\"");
    }

    #[test]
    fn quote_arguments_with_backslashes() {
        let cmd = OsStr::new("prog.exe");
        let args = vec![OsString::from("abc\\")];
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);
        // Backslashes alone are not enough to trigger quoting.
        assert_eq!(s, "prog.exe abc\\");
    }

    #[test]
    fn quote_arguments_quoted_trailing_backslashes_are_doubled() {
        let cmd = OsStr::new("prog.exe");
        let args = vec![OsString::from("a b\\")]; // one trailing backslash
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);
        // Expect the trailing backslash to be doubled inside quotes
        assert_eq!(s, "prog.exe \"a b\\\\\"" );
    }

    #[test]
    fn quote_arguments_backslashes_before_quote_escaped() {
        let cmd = OsStr::new("prog.exe");
        // Two backslashes followed by a quote
        let args = vec![OsString::from("a\\\\\"b")]; // a\\"b
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);

        assert_eq!(s, "prog.exe \"a\\\\\\\\\\\"b\""); // a\\\\\"b
    }

    #[test]
    fn quote_arguments_internal_backslashes() {
        let cmd = OsStr::new("pr og.exe");
        let args = vec![OsString::from("\\some\\directory with\\spaces"), OsString::from("argument2")];
        let out = quote_arguments(cmd, &args).expect("quoting should succeed");
        let s = utf16_to_string(&out);

        assert_eq!(s, "\"pr og.exe\" \"\\some\\directory with\\spaces\" argument2");
    }
}
