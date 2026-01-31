// SPDX-License-Identifier: MIT

use std::io::{Read, Write};

fn main() {
    let filename = std::env::args().nth(0).unwrap();
    std::io::stderr().write_all(format!("Reading file: {}\n", filename).as_bytes()).unwrap();

    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    let mut buf = [0u8];
    stdin.read_exact(&mut buf).unwrap();
    // Don't need to check the value.

    buf[0] = b'1';
    stdout.write_all(&buf).unwrap();
    stdout.flush().unwrap();
    std::fs::read_to_string(filename).unwrap();

    buf[0] = b'2';
    stdout.write_all(&buf).unwrap();
    stdout.flush().unwrap();
}
