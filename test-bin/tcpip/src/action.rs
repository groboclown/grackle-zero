// SPDX-License-Identifier: MIT

use super::debug::debug;
use std::io::ErrorKind;
use std::net::TcpStream;

pub(crate) fn perform(addr: String) {
    debug(format!("opening TCP/IP connection to {}", addr));

    // Just open the connection.  Don't read or write to it.
    // Opening the connection alone should be blocked.
    match TcpStream::connect(addr.as_str()) {
        Ok(_) => (),

        Err(e) => match e.kind() {
            // Some errors are just bad TCP/IP setup issues, not the OS blocking.
            // Panic on OS blocking, and let TCP/IP setup issues slide.
            ErrorKind::AddrInUse => debug(format!("Allowing {:?}", e)),
            ErrorKind::AddrNotAvailable => debug(format!("Allowing {:?}", e)),
            ErrorKind::BrokenPipe => debug(format!("Allowing {:?}", e)),
            ErrorKind::ConnectionAborted => debug(format!("Allowing {:?}", e)),
            ErrorKind::ConnectionRefused => debug(format!("Allowing {:?}", e)),
            ErrorKind::ConnectionReset => debug(format!("Allowing {:?}", e)),
            ErrorKind::Interrupted => debug(format!("Allowing {:?}", e)),
            ErrorKind::InvalidInput => debug(format!("Allowing {:?}", e)),
            ErrorKind::NetworkDown => debug(format!("Allowing {:?}", e)),
            ErrorKind::NetworkUnreachable => debug(format!("Allowing {:?}", e)),
            ErrorKind::NotConnected => debug(format!("Allowing {:?}", e)),
            ErrorKind::ResourceBusy => debug(format!("Allowing {:?}", e)),
            ErrorKind::WouldBlock => debug(format!("Allowing {:?}", e)),

            // This is what we expect:
            // PermissionDenied
            // But we'll panic on any other.
            _ => {
                panic!("Assuming this is the OS blocking the request: {:?}", e);
            }
        },
    }
}
