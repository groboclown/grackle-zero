//! Event transmission and receiving.
//!
//! Handles sending an event packet, and receiving an event packet.

/// The header for event packets.
/// TODO fix up the size to be constants, throughout this source.
/// TODO simplify the names.
pub struct EventPacketHeader {
    pub packet_id: [u8; EVENT_PACKET_HEADER_PACKET_ID_SIZE],
    pub cmd_packet_id: [u8; 8],
    pub event_id: [u8; 12],
    pub size: usize,
}

const EVENT_PACKET_HEADER_PACKET_ID_SIZE: usize = 8;
const _HEADER_PACKET_ID_POS_START: usize = 0;
const _HEADER_PACKET_ID_POS_END: usize =
    _HEADER_PACKET_ID_POS_START + EVENT_PACKET_HEADER_PACKET_ID_SIZE;
const _HEADER_CMD_PACKET_ID_POS_START: usize = _HEADER_PACKET_ID_POS_END;
const _HEADER_CMD_PACKET_ID_POS_END: usize = _HEADER_CMD_PACKET_ID_POS_START + 8;
const _HEADER_EVENT_ID_POS_START: usize = _HEADER_CMD_PACKET_ID_POS_END;
const _HEADER_EVENT_ID_POS_END: usize = _HEADER_EVENT_ID_POS_START + 12;
const _HEADER_SIZE_POS_START: usize = _HEADER_EVENT_ID_POS_END;
const _HEADER_SIZE_POS_END: usize = _HEADER_SIZE_POS_START + 4;
const _HEADER_COUNT: usize = _HEADER_SIZE_POS_END;
const _HEADER_PAYLOAD_POS_START: usize = _HEADER_SIZE_POS_END;

/// The full event packet.
/// The payload length must match the header's size value.
pub struct EventPacket {
    pub header: EventPacketHeader,
    pub payload: Vec<u8>,
}

/// Handles reading events.
pub struct EventReader {
    max_payload_size: usize,
}

const _BUFFER_SIZE: usize = 8 * 1024;

impl EventReader {
    pub fn new(max_payload_size: usize) -> Self {
        EventReader { max_payload_size }
    }

    /// Read the next event packet from the stream.
    pub fn read<R: std::io::Read>(self, source: &mut R) -> Result<EventPacket, std::io::Error> {
        let mut header_buff: [u8; _HEADER_COUNT] = [0; _HEADER_COUNT];
        source.read_exact(&mut header_buff)?;
        let size = header_size(&header_buff, self.max_payload_size)?;

        let mut remaining = size;
        let mut payload = Vec::with_capacity(size);
        let mut buff: [u8; _BUFFER_SIZE] = [0; _BUFFER_SIZE];
        while remaining > 0 {
            let read_count = std::cmp::min(_BUFFER_SIZE, remaining);
            match source.read_exact(&mut buff[0..read_count]) {
                Ok(_) => (),
                Err(e) => {
                    return Err(e);
                }
            };
            payload.extend_from_slice(&buff[0..read_count]);
            remaining -= read_count;
        }
        Ok(EventPacket {
            header: EventPacketHeader {
                packet_id: header_packet_id(&header_buff),
                cmd_packet_id: header_cmd_packet_id(&header_buff),
                event_id: header_event_id(&header_buff),
                size,
            },
            payload,
        })
    }
}

/// Handles writing events.
pub struct EventWriter {}

impl EventWriter {
    pub fn new() -> Self {
        EventWriter {}
    }

    /// Writes the packet to the stream.
    ///
    /// This writes the packet exactly as specified in the header.
    /// If the payload does not match the header's size, then this
    /// returns an error without writing anything.
    ///
    /// The writer is flushed after the packet is written.
    pub fn write<W: std::io::Write>(
        self,
        out: &mut W,
        packet: &EventPacket,
    ) -> Result<(), std::io::Error> {
        if packet.header.size != packet.payload.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "header size != payload size",
            ));
        }
        let header_size = size_to_octets(packet.header.size)?;
        out.write_all(&packet.header.packet_id)?;
        out.write_all(&packet.header.cmd_packet_id)?;
        out.write_all(&packet.header.event_id)?;
        out.write_all(&header_size)?;

        let chunks: (&[[u8; _BUFFER_SIZE]], &[u8]) = packet.payload.as_chunks();
        for p in chunks.0 {
            out.write_all(p)?;
        }
        out.write_all(chunks.1)?;

        Ok(())
    }

    /// Write the event, with the event ID as a &str.
    ///
    /// If the event string is larger than the maximum length (12),
    /// it returns an error.  If it's less than the length, then it is
    /// zero padded.
    ///
    /// The packet IDs are turned into big-endian formatted bytes.
    pub fn write_event_str<'a, 'b, W: std::io::Write>(
        self,
        out: &'b mut W,
        packet_id: u64,
        cmd_packet_id: u64,
        event: &'a str,
        payload: Vec<u8>,
    ) -> Result<(), std::io::Error> {
        let mut header = EventPacketHeader {
            packet_id: packet_id.to_be_bytes(),
            cmd_packet_id: cmd_packet_id.to_be_bytes(),
            event_id: [0; 12],
            size: payload.len(),
        };
        let evt_bytes = event.as_bytes();
        let evt_size = std::cmp::min(12, evt_bytes.len());
        for i in 0..evt_size {
            header.event_id[i] = evt_bytes[i];
        }
        for i in evt_size..12 {
            header.event_id[i] = 0;
        }
        self.write(out, &EventPacket { header, payload })
    }
}

fn header_packet_id(header: &[u8; _HEADER_COUNT]) -> [u8; 8] {
    [
        header[_HEADER_PACKET_ID_POS_START + 0],
        header[_HEADER_PACKET_ID_POS_START + 1],
        header[_HEADER_PACKET_ID_POS_START + 2],
        header[_HEADER_PACKET_ID_POS_START + 3],
        header[_HEADER_PACKET_ID_POS_START + 4],
        header[_HEADER_PACKET_ID_POS_START + 5],
        header[_HEADER_PACKET_ID_POS_START + 6],
        header[_HEADER_PACKET_ID_POS_START + 7],
    ]
}

fn header_cmd_packet_id(header: &[u8; _HEADER_COUNT]) -> [u8; 8] {
    [
        header[_HEADER_CMD_PACKET_ID_POS_START + 0],
        header[_HEADER_CMD_PACKET_ID_POS_START + 1],
        header[_HEADER_CMD_PACKET_ID_POS_START + 2],
        header[_HEADER_CMD_PACKET_ID_POS_START + 3],
        header[_HEADER_CMD_PACKET_ID_POS_START + 4],
        header[_HEADER_CMD_PACKET_ID_POS_START + 5],
        header[_HEADER_CMD_PACKET_ID_POS_START + 6],
        header[_HEADER_CMD_PACKET_ID_POS_START + 7],
    ]
}

fn header_event_id(header: &[u8; _HEADER_COUNT]) -> [u8; 12] {
    [
        header[_HEADER_EVENT_ID_POS_START + 0],
        header[_HEADER_EVENT_ID_POS_START + 1],
        header[_HEADER_EVENT_ID_POS_START + 2],
        header[_HEADER_EVENT_ID_POS_START + 3],
        header[_HEADER_EVENT_ID_POS_START + 4],
        header[_HEADER_EVENT_ID_POS_START + 5],
        header[_HEADER_EVENT_ID_POS_START + 6],
        header[_HEADER_EVENT_ID_POS_START + 7],
        header[_HEADER_EVENT_ID_POS_START + 8],
        header[_HEADER_EVENT_ID_POS_START + 9],
        header[_HEADER_EVENT_ID_POS_START + 10],
        header[_HEADER_EVENT_ID_POS_START + 11],
    ]
}

fn header_size_octets(header: &[u8; _HEADER_COUNT]) -> [u8; 4] {
    [
        header[_HEADER_SIZE_POS_START + 0],
        header[_HEADER_SIZE_POS_START + 1],
        header[_HEADER_SIZE_POS_START + 2],
        header[_HEADER_SIZE_POS_START + 3],
    ]
}

fn header_size(header: &[u8; _HEADER_COUNT], max_size: usize) -> Result<usize, std::io::Error> {
    let u32_size = u32::from_be_bytes(header_size_octets(&header));
    let size: usize = u32_size
        .try_into()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if size > max_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "packet size too large",
        ));
    }
    Ok(size)
}

fn size_to_octets(size: usize) -> Result<[u8; 4], std::io::Error> {
    let u32_size = u32::try_from(size)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    Ok(u32_size.to_be_bytes())
}

#[cfg(test)]
mod tests {
    #![allow(const_item_mutation)]
    use super::*;

    const ZERO_SIZE_EVENT: &[u8] = &[
        // Packet ID: 8 bytes
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        //
        // Cmd Packet ID: 8 bytes
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, //
        //
        // Event ID: 12 bytes
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c,
        //
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
        let data = EventReader::new(10).read(&mut ZERO_SIZE_EVENT).unwrap();
        assert_eq!(data.header.packet_id, ZERO_SIZE_EVENT[0..8]);
        assert_eq!(data.header.cmd_packet_id, ZERO_SIZE_EVENT[8..16]);
        assert_eq!(data.header.event_id, ZERO_SIZE_EVENT[16..28]);
        assert_eq!(data.header.size, 0);
        assert_eq!(data.payload.len(), 0);
    }

    #[test]
    fn test_write_zero_bytes() {
        let mut packet_id = [0u8; _HEADER_PACKET_ID_POS_END - _HEADER_PACKET_ID_POS_START];
        packet_id.copy_from_slice(
            &ZERO_SIZE_EVENT[_HEADER_PACKET_ID_POS_START.._HEADER_PACKET_ID_POS_END],
        );
        let mut cmd_packet_id =
            [0u8; _HEADER_CMD_PACKET_ID_POS_END - _HEADER_CMD_PACKET_ID_POS_START];
        cmd_packet_id.copy_from_slice(
            &ZERO_SIZE_EVENT[_HEADER_CMD_PACKET_ID_POS_START.._HEADER_CMD_PACKET_ID_POS_END],
        );
        let mut event_id = [0u8; _HEADER_EVENT_ID_POS_END - _HEADER_EVENT_ID_POS_START];
        event_id.copy_from_slice(
            &ZERO_SIZE_EVENT[_HEADER_EVENT_ID_POS_START.._HEADER_EVENT_ID_POS_END],
        );
        let mut out: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());
        EventWriter::new()
            .write(
                &mut out,
                &EventPacket {
                    header: EventPacketHeader {
                        packet_id,
                        cmd_packet_id,
                        event_id,
                        size: 0,
                    },
                    payload: vec![],
                },
            )
            .unwrap();
        let data = out.get_ref();
        assert_eq!(data.eq(&ZERO_SIZE_EVENT[0.._HEADER_COUNT]), true);
    }
}
