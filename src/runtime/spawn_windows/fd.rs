//! Handle the "file descriptor" style passing from the parent to the child.

use std::fs::File;
use std::os::windows::io::FromRawHandle;
use windows_result::HRESULT;
use windows_sys::Win32::System::Console;

use windows::Win32::{
    Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, FALSE, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, INVALID_HANDLE_VALUE, SetHandleInformation},
    Security, System::{Pipes, Threading::GetCurrentProcess},
};


pub struct WinFdSet {
    pub stdin: StdIoFd,
    pub stdout: StdIoFd,
    pub stderr: StdIoFd,
    pub others: Vec<WinFd>,
}

impl WinFdSet {
    pub fn new(stdio: StdIoSet, others: Vec<WinFd>) -> windows::core::Result<Self> {
        let stdin = match stdio.stdin {
            StdIo::Pipe => StdIoFd::Pipe(WinFd::new(0, StreamDirection::ToChild)?),
            StdIo::None => StdIoFd::None,
            StdIo::PassThrough => StdIoFd::Pipe(WinFd::from_std(0)?),
        };
        let stdout = match stdio.stdout {
            StdIo::Pipe => StdIoFd::Pipe(WinFd::new(1, StreamDirection::FromChild)?),
            StdIo::None => StdIoFd::None,
            StdIo::PassThrough => StdIoFd::Pipe(WinFd::from_std(1)?),
        };
        let stderr = match stdio.stderr {
            StdIo::Pipe => StdIoFd::Pipe(WinFd::new(2, StreamDirection::FromChild)?),
            StdIo::None => StdIoFd::None,
            StdIo::PassThrough => StdIoFd::Pipe(WinFd::from_std(2)?),
        };
        Ok(WinFdSet { stdin, stdout, stderr, others })
    }
}


#[derive(Debug, Clone, Copy)]
pub enum StreamDirection {
    ToChild,
    FromChild,
}

/// Piped file descriptor.
pub struct WinFd {
    fd: u32,
    parent_handle: Option<HANDLE>,
    child_handle: Option<HANDLE>,
    direction: StreamDirection,
}

impl WinFd {
    pub fn fd(&self) -> u32 {
        self.fd
    }
}

pub struct StdIoSet {
    pub stdin: StdIo,
    pub stdout: StdIo,
    pub stderr: StdIo,
}

pub enum StdIo {
    None,         // don't use this fd
    PassThrough,  // reuse the parent's handle
    Pipe,         // use a pipe.
}

pub enum StdIoFd {
    None,         // don't use this fd
    Pipe(WinFd),  // use a pipe.
}



const DEFAULT_BUFFER_SIZE: u32 = 0;  // use default buffer size


impl WinFd {
    /// Create the piped handles to represent the file descriptor.
    /// Also, prepares the handles for correct inheritable flag setup.
    pub fn new(fd: u32, direction: StreamDirection) -> windows::core::Result<Self> {
        // Create all pairs a non-inheritable, then swap it on when ready to run the jail.
        let sa = Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<Security::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: FALSE, // create non-inheritable handles
        };

        let mut read = HANDLE::default();
        let mut write = HANDLE::default();

        unsafe {
            Pipes::CreatePipe(
                &mut read, // hReadPipe (writes to the variable)
                &mut write, // hWritePipe (writes to the variable)
                Some(&sa), // lpPipeAttributes (controls inheritability)
                DEFAULT_BUFFER_SIZE,  // nSize (0 means use default)
            )?;
        }

        Ok(match direction {
            StreamDirection::ToChild => {
                allow_inheritable(read)?;
                deny_inheritable(write)?;
                Self { fd, direction, parent_handle: Some(write), child_handle: Some(read) }
            },
            StreamDirection::FromChild => {
                allow_inheritable(write)?;
                deny_inheritable(read)?;
                Self { fd, direction, parent_handle: Some(read), child_handle: Some(write) }
            },
        })
    }

    fn from_std(fd: u32) -> windows::core::Result<Self> {
        let (direction, std_handle) = match fd {
            0 => (StreamDirection::ToChild, Console::STD_INPUT_HANDLE),
            1 => (StreamDirection::FromChild, Console::STD_OUTPUT_HANDLE),
            2 => (StreamDirection::FromChild, Console::STD_ERROR_HANDLE),
            _ => { return Err(windows::core::Error::new(HRESULT(0i32), "invalid standard i/o number")); },
        };

        // Duplicate the handle, and use that duplicated handle just like a WinFd.
        // This is safer by only setting the allow-inherit on the duplicated handle,
        // rather than on the parent, which can cause problems if multiple sandboxes are
        // launched with different I/O requirements.
        let parent = unsafe { Console::GetStdHandle(std_handle) };
        let null: *mut std::ffi::c_void = core::ptr::null::<*mut std::ffi::c_void>() as *mut std::ffi::c_void;
        // Some environments don't have a console.
        if parent == null || parent == INVALID_HANDLE_VALUE.0 {
            return Err(windows::core::Error::from_thread())
        }
        let mut child = HANDLE::default();
        unsafe { DuplicateHandle(
            GetCurrentProcess(), // source process
            HANDLE(parent), // source handle
            GetCurrentProcess(),  // target process (still this process, because inheritence will do its thang)
            &mut child,  // target handle
            0,  // desired access: 0 means same access as the source
            true,  // inherit-handle: true, because the child may inherit it.
            DUPLICATE_SAME_ACCESS,  // options
        )? };
        Ok(Self {
            fd,
            direction,
            parent_handle: None, // This is a pass-through FD, so the parent process will not access it.
            child_handle: Some(child),
        })
    }

    /// Export the child handle as an environment-variable or argument capable encoded string.
    /// This will format it like `FD_NUMBER:0xHANDLE_ADDRESS;`, looking something like:
    /// `1:0x00000000000001F4;`
    pub fn as_env_val(&self) -> Option<std::ffi::OsString> {
        match self.child_handle {
            None => None,
            Some(h) => {
                let mut ret = std::ffi::OsString::new();
                ret.push(format!("{}:0x{:x};", self.fd, h.0 as usize));
                Some(ret)
            }
        }
    }

    pub fn child(&self) -> Option<HANDLE> {
        self.child_handle
    }

    // Takes the parent handle as a stream reader.
    pub fn as_reader(&mut self) -> Option<Box<dyn std::io::Read>> {
        let handle = match self.parent_handle.take() {
            None => { return None; }
            Some(e) => e,
        };
        match self.direction {
            StreamDirection::ToChild => None,
            StreamDirection::FromChild => Some(Box::new(unsafe { File::from_raw_handle(handle.0) }))
        }
    }

    // Takes the parent handle as a stream writer.
    pub fn as_writer(&mut self) -> Option<Box<dyn std::io::Write>> {
        let handle = match self.parent_handle.take() {
            None => { return None; }
            Some(e) => e,
        };
        match self.direction {
            StreamDirection::FromChild => None,
            StreamDirection::ToChild => Some(Box::new(unsafe { File::from_raw_handle(handle.0) }))
        }
    }
}

impl Drop for WinFd {
    fn drop(&mut self) {
        unsafe {
            match self.parent_handle.take() {
                None => (),
                Some(h) => {
                    let _ = CloseHandle(h);
                }
            };
            match self.child_handle.take() {
                None => (),
                Some(h) => {
                    let _ = CloseHandle(h);
                }
            };
        }
    }
}


/// Prepare windows handle for inherting into the child sandbox.
fn allow_inheritable(allow: HANDLE) -> windows::core::Result<()> {
    unsafe { SetHandleInformation(allow, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
    Ok(())
}


/// Prepare windows handle for NOT inherting into the child sandbox.
fn deny_inheritable(deny: HANDLE) -> windows::core::Result<()> {
    // Clear inheritability on known "deny" handles
    unsafe { SetHandleInformation(deny, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))? };
    Ok(())
}
