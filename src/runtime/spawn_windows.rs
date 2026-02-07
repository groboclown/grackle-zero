// SPDX-License-Identifier: MIT

//! Sandbox for Windows.
//! 
//! Inspired by the Chromium sandboxing model.
//! Code:
//!   https://github.com/chromium/chromium/blob/main/sandbox
//!   under the BSD license.
//! Documentation:
//!   https://github.com/chromium/chromium/blob/main/docs/design/sandbox.md
//!   https://github.com/chromium/chromium/blob/main/docs/design/sandbox_faq.md

mod launch_quote;
mod fd;
mod launch;
mod monitor;
mod jail;

pub(crate) use launch::launch_child;
