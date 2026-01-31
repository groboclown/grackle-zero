// SPDX-License-Identifier: MIT

mod action;
mod debug;

use std::io::{Read, Write};

fn main() {
    let arg = std::env::args().nth(0).unwrap();
    debug::debug(format!("started [{}] [{}]", file!(), arg));
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    // 1. Read the message from the parent to indicate ready to start.
    let mut buf = [0u8];
    stdin.read_exact(&mut buf).unwrap();
    // Don't need to check the value.  It should be '0'.

    // 2. Tell the parent that the action is going to start.
    buf[0] = b'1';
    stdout.write_all(&buf).unwrap();
    stdout.flush().unwrap();

    // 3. Perform the operation.
    action::perform(arg);

    // 4. Tell the parent that the operation completed.
    buf[0] = b'2';
    stdout.write_all(&buf).unwrap();
    stdout.flush().unwrap();
}
