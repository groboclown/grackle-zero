// SPDX-License-Identifier: MIT

//! Spawns the process with proper security restrictions.
//! Specific to Linux.  Uses Landlock for jail restrictions.

mod dependencies;
mod fd;
mod jail;
mod launch;

pub(crate) use launch::launch_child;
