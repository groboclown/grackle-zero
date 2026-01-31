//! Common error type.
//!

use std::{ffi::NulError, fmt::Display};

#[derive(Debug)]
pub enum SandboxError {
    Io(std::io::Error),
    ProcessError(String),
    JailSetup(String),
    JailNotSupported(String),
}

impl Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sandbox error")
    }
}

impl From<std::io::Error> for SandboxError {
    fn from(e: std::io::Error) -> Self {
        SandboxError::Io(e)
    }
}

impl From<which::Error> for SandboxError {
    fn from(e: which::Error) -> Self {
        SandboxError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e))
    }
}

impl From<NulError> for SandboxError {
    fn from(e: NulError) -> Self {
        SandboxError::Io(std::io::Error::new(std::io::ErrorKind::InvalidFilename, e))
    }
}

impl Into<std::io::Error> for SandboxError {
    fn into(self) -> std::io::Error {
        match self {
            Self::Io(e) => e,
            Self::ProcessError(e) => std::io::Error::new(std::io::ErrorKind::Unsupported, e),
            Self::JailSetup(e) => std::io::Error::new(std::io::ErrorKind::InvalidInput, e),
            Self::JailNotSupported(e) => std::io::Error::new(std::io::ErrorKind::NotSeekable, e),
        }
    }
}
