//! Read & Write utility functions.

/// Number of octets (bytes) in a u32.
pub const U32_SIZE: usize = size_of::<u32>();

/// Convert the slice of U32_SIZE u8 into a u32, using big endian encoding.
#[inline]
pub fn get_be_u32(buff: &[u8]) -> u32 {
    let buff: [u8; U32_SIZE] = buff.try_into().unwrap();
    u32::from_be_bytes(buff)
}

/// Put the u32, as big endian, into the slice.
pub fn set_be_u32(value: u32, buff: &mut [u8]) {
    let b = value.to_be_bytes();
    for i in 0..U32_SIZE {
        buff[i] = b[i];
    }
}

/// Read the `count` number of bytes from the reader in chunks.
pub fn read_chunked_bytes<'a, 'b, R: std::io::Read, const COUNT: usize>(
    source: &'a mut R,
    count: usize,
    buff: &'b mut [u8; COUNT],
) -> Result<Vec<u8>, std::io::Error> {
    let mut payload = Vec::with_capacity(count);
    let mut count = count;
    while count > 0 {
        let read_count = std::cmp::min(COUNT, count);
        match source.read_exact(&mut buff[0..read_count]) {
            Ok(_) => (),
            Err(e) => {
                return Err(e);
            }
        };
        payload.extend_from_slice(&buff[0..read_count]);
        count -= read_count;
    }
    Ok(payload)
}

/// Write the data to the stream in chunks.
///
/// Example of using this:
///
/// const SIZE_8K: usize = 8 * 1024;
///
/// fn write_8k_chunks<'a, 'b, W: std::io::Write>(
///     out: &'a mut W,
///     data: &'b Vec<u8>,
/// ) -> Result<(), std::io::Error> {
///     write_chunked::<W, SIZE_8K>(out, data)
/// }
pub fn write_chunked<'a, 'b, W: std::io::Write, const COUNT: usize>(
    out: &'a mut W,
    data: &'b Vec<u8>,
) -> Result<(), std::io::Error> {
    let chunks: (&[[u8; COUNT]], &[u8]) = data.as_chunks();
    for p in chunks.0 {
        out.write_all(p)?;
    }
    out.write_all(chunks.1)?;
    Ok(())
}
