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
//!   https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/SecAuthZ/implementing-an-appcontainer.md

mod appcontainer;
mod attribute_list;
mod conv;
mod error;
mod fd;
mod launch;
mod launch_quote;
mod monitor;
mod process_token;
mod jail;

pub(crate) use launch::launch_child;
