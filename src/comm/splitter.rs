//! Split a stream based on a u8 value.
use super::rwutil;

/// Read from the stream up to the separator, or the maximum length value.
///
/// On standard exit, it returns the read bytes + whether the separator was found (true)
/// or the length was encountered (false).
///
/// While not required, the reader should be a buffered reader for better performance.
pub fn read_next<R: std::io::Read>(
    source: &mut R,
    sep: u8,
    max_len: usize,
) -> Result<(Vec<u8>, bool), std::io::Error> {
    let mut buf = [0];
    let mut ret = vec![];
    let mut count = 0;
    let mut sep_found = false;
    while count < max_len {
        source.read_exact(&mut buf)?;
        if buf[0] == sep {
            sep_found = true;
            break;
        }
        ret.push(buf[0]);
        count += 1;
    }
    Ok((ret, sep_found))
}

const _BUF_SIZE: usize = 8 * 1024;

/// Write the next item to the stream plus the separator.
pub fn write_next<W: std::io::Write>(
    out: &mut W,
    data: &Vec<u8>,
    sep: u8,
) -> Result<(), std::io::Error> {
    rwutil::write_chunked::<W, _BUF_SIZE>(out, data)?;
    out.write_all(&[sep])?;
    out.flush()
}
