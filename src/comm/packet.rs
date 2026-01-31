//! Common traits for working with cross-process packets in communications.

/// Generic u8 data packet (sometimes called an "envelope").
///
/// Packets consist of a header plus a payload.
pub struct U8Packet<H> {
    pub header: H,
    pub payload: Vec<u8>,
}

/// Reads U8Packet objects from a byte stream.
pub trait U8PacketRead<H> {
    /// Read the next event packet from the stream.
    fn read<'a, R: std::io::Read>(&self, source: &'a mut R) -> Result<U8Packet<H>, std::io::Error>;
}

/// Writes U8Packet objects to a byte stream.
pub trait U8PacketWrite<H> {
    /// Writes U8Packet objects to a byte stream.
    ///
    /// The implementation should perform validation on the
    /// packet's contents.  However, some implementations may skip this for
    /// performance reasons, and, in doing so, should clearly document that
    /// it skips validation.
    ///
    /// At the end of a successful packet write, this write call must flush the stream.
    fn write<'a, 'b, W: std::io::Write>(
        &self,
        out: &'a mut W,
        packet: &'b U8Packet<H>,
    ) -> Result<(), std::io::Error>;
}
