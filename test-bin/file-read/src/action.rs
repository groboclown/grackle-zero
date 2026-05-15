// SPDX-License-Identifier: MIT

use super::debug::debug;

pub(crate) fn perform(filename: String) {
    debug(format!("reading from {}", filename));
    let _ = std::fs::read_to_string(filename).unwrap();
}
