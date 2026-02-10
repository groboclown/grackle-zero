// SPDX-License-Identifier: MIT

//! Windows based errors.

use crate::runtime::error::SandboxError;

/// Allows handling Windows errors and library errors in the same code.
#[derive(Debug)]
pub enum WindowsSandboxError {
    Sandbox(SandboxError),
    Setup(windows::core::Error),
    Run(windows::core::Error),
}

impl WindowsSandboxError {
    pub fn setup_message(reason: &str) -> Self {
        WindowsSandboxError::Sandbox(SandboxError::JailSetup(reason.to_string()))
    }

    pub fn setup(e: windows::core::Error) -> Self {
        WindowsSandboxError::Setup(e)
    }

    pub fn run(e: windows::core::Error) -> Self {
        WindowsSandboxError::Run(e)
    }
}

impl From<windows::core::Error> for WindowsSandboxError {
    fn from(value: windows::core::Error) -> Self {
        WindowsSandboxError::Setup(value)
    }
}

impl Into<SandboxError> for WindowsSandboxError {
    fn into(self) -> SandboxError {
        match self {
            Self::Sandbox(s) => s,
            Self::Setup(e) => SandboxError::JailSetup(format!("problem setting up the process: {:?}", e)),
            Self::Run(e) => SandboxError::ProcessError(format!("problem handling process: {:?}", e)),
        }
    }
}
