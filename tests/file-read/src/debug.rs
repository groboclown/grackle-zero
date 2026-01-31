// SPDX-License-Identifier: MIT

use std::io::Write;

pub(crate) fn debug(m: String) {
    std::io::stderr().write_all(b"[CHILD] ").unwrap();
    std::io::stderr().write_all(&m.into_bytes()).unwrap();
    std::io::stderr().write_all(b"\n").unwrap();
}
