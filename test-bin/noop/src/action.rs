// SPDX-License-Identifier: MIT

use super::debug::debug;

pub(crate) fn perform(arg: String) {
    debug(format!("doing nothing with {}", arg));
}
