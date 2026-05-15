// SPDX-License-Identifier: MIT

use super::debug::debug;

pub(crate) fn perform(action: String) {
    // Hopefully, the program stops here, while trying to access the clipboard.
    let mut clipboard = arboard::Clipboard::new().expect("could not get the clipboard");
    let loop_count: u64 = action.parse().expect("argument must be number of times to loop");
    let mut idx = 0;
    let mut discovered_count = 0;
    while idx < loop_count {
        idx += 1;

        // Try to get text.
        match handle_err(clipboard.get_text()) {
            CbState::Available => {
                debug(format!("read clipboard text"));
                discovered_count += 1;
            }
            CbState::OtherType => {
                // Ensure it can be captured.
                if let CbState::Empty = handle_err(clipboard.get_image()) {
                    // Nothing to capture.  False other type?
                    debug(format!("read clipboard image"));
                } else {
                    discovered_count += 1;
                }
            }
            CbState::Empty => (),
        }

        if idx < loop_count {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    // Discovering clipboard content means this did what it was supposed to do.
    if discovered_count == 0 {
        panic!("discovered no content in the clipboard");
    }
}

enum CbState {
    Available,
    OtherType,
    Empty,
}

fn handle_err<A>(r: Result<A, arboard::Error>) -> CbState {
    match r {
        Ok(_) => CbState::Available,
        Err(e) => match e {
            // Either empty or contains an incompatible format.
            arboard::Error::ContentNotAvailable => {
                debug(format!("empty clipboard"));
                CbState::Empty
            }

            // OS or environment doesn't support the clipboard.
            // We want this error, as it can mean security controls blocked access.
            arboard::Error::ClipboardNotSupported => { panic!("clipboard not supported"); }

            // Another party holds the clipboard.
            // In some circumstances, Windows can use this as the message to mean no access
            // without reporting an error.
            arboard::Error::ClipboardOccupied => {
                debug(format!("clipboard in use by another process"));
                CbState::Empty
            }
            
            // Clipboard contains an image of an unsupported format type.
            arboard::Error::ConversionFailure => CbState::OtherType,

            arboard::Error::Unknown{ description: d } => { panic!("clipboard access error: {}", d); }

            // The enum is non-exhaustive
            other => { panic!("unexpected clipboard error: {}", other); }
        }
    }
}
