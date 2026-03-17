// SPDX-License-Identifier: MIT

//! # grackle-zero
//!
//! The library that runs child programs with zero OS permissions.

pub mod comm;
pub mod restrictions;
pub mod runtime;
pub mod macros;

pub use runtime::{Child, CommHandler, FdMode, FdSet, LaunchEnv, sandbox_child};
pub use restrictions::{create_compat_restrictions, create_strict_restrictions, Restrictions};
