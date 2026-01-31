//! Decode the data stream as a "sized packet", where it has an envelope containing only the size of the payload.

use super::packet;
use super::rwutil;

pub struct SizeHeader {
    pub size: usize,
}

const _HEADER_SIZE_START: usize = 0;
const _HEADER_SIZE_LEN: usize = size_of::<u32>();
const _HEADER_SIZE_END: usize = _HEADER_SIZE_START + _HEADER_SIZE_LEN;

/// Number of octets in the SizeHeader.
const HEADER_LEN: usize = _HEADER_SIZE_END;

/// Maximum payload size allowed by the header.
pub const MAX_PAYLOAD_SIZE: usize = u32::MAX as usize;

/// The full event packet that uses a size envelope.
pub type SizePacket = packet::U8Packet<SizeHeader>;

/// Handles reading SizePacket values.
///
/// While the size has a theoretical maximum of 2^32 octets (4 GB),
/// implementations should put a practical cap on this.
pub struct SizePacketRead {
    max_payload_size: usize,
}

impl SizePacketRead {
    pub fn new(max_payload_size: usize) -> Self {
        if max_payload_size > MAX_PAYLOAD_SIZE {
            // This is a panic, as the packet size maximum should be established as
            // part of the communication protocol, thus a bug.
            panic!("max_payload_size beyond maximum capability of packet");
        }
        SizePacketRead { max_payload_size }
    }
}

const PACKET_BUFFER_SIZE: usize = 8 * 1024;

impl packet::U8PacketRead<SizeHeader> for SizePacketRead {
    fn read<R: std::io::Read>(
        &self,
        source: &mut R,
    ) -> Result<packet::U8Packet<SizeHeader>, std::io::Error> {
        let mut header_buff: [u8; HEADER_LEN] = [0; HEADER_LEN];
        source.read_exact(&mut header_buff)?;
        let size = rwutil::get_be_u32(&header_buff[_HEADER_SIZE_START.._HEADER_SIZE_END]) as usize;
        if size > self.max_payload_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "payload size exceeded packet maximum",
            ));
        }
        let header = SizeHeader { size };

        let mut buff = [0u8; PACKET_BUFFER_SIZE];
        let payload = rwutil::read_chunked_bytes(source, size, &mut buff)?;
        let packet = packet::U8Packet { header, payload };
        Ok(packet)
    }
}

/// Handles writing SizePacket values.
pub struct SizePacketWrite {}

impl SizePacketWrite {
    pub fn new() -> Self {
        SizePacketWrite {}
    }
}

const _SIZE_8K: usize = 8 * 1024;

impl packet::U8PacketWrite<SizeHeader> for SizePacketWrite {
    fn write<'a, 'b, W: std::io::Write>(
        &self,
        out: &'a mut W,
        packet: &'b packet::U8Packet<SizeHeader>,
    ) -> Result<(), std::io::Error> {
        // Validate the packet.
        if packet.header.size != packet.payload.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "header size != payload size",
            ));
        }

        out.write_all(&(packet.header.size as u32).to_be_bytes())?;
        rwutil::write_chunked::<W, _SIZE_8K>(out, &packet.payload)?;

        // Finish with flushing the writer.
        out.flush()
    }
}

#[cfg(test)]
mod tests {
    #![allow(const_item_mutation)]

    use crate::comm::packet::U8PacketWrite;

    use super::super::packet::U8PacketRead;
    use super::*;

    const ZERO_SIZE_EVENT: &[u8] = &[
        // Payload size: 4 bytes
        0x00, 0x00, 0x00, 0x00,
        //
        // Payload: 0 bytes
        //
        // Some extra data to ensure EOF isn't incorrectly handled.
        0x99,
    ];

    #[test]
    fn test_read_zero_bytes() {
        let r = SizePacketRead::new(10);
        let data = &r.read(&mut ZERO_SIZE_EVENT).unwrap();
        assert_eq!(data.header.size, 0);
        assert_eq!(data.payload.len(), 0);
    }

    #[test]
    fn test_write_zero_bytes() {
        let mut out: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());
        SizePacketWrite::new()
            .write(
                &mut out,
                &SizePacket {
                    header: SizeHeader { size: 0 },
                    payload: vec![],
                },
            )
            .unwrap();
        let data = &out.get_ref()[..out.position() as usize];
        assert!(data.eq(&ZERO_SIZE_EVENT[0..HEADER_LEN]), "found: {:02X?}, expected: {:02X?}", data, &ZERO_SIZE_EVENT[0..HEADER_LEN]);
    }
}
