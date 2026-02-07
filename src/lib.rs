// SPDX-License-Identifier: MIT

//! # grackle-zero
//!
//! The library that runs child programs with zero OS permissions.

pub mod comm;
pub mod runtime;

pub use runtime::{Child, CommHandler, FdMode, FdSet, LaunchEnv, sandbox_child};

#[cfg(test)]
mod integration_tests;
