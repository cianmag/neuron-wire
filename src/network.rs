//! Network transport layer for the neuron protocol.
//!
//! Implements framing for zero-copy message transport over any stream.
//! Each message is prefixed with a 4-byte length header:
//!
//! ```text
//! [0-3]   frame_len: u32   = length of the NWP message (excludes this field)
//! [4-19]  MessageHeader    = 16-byte zero-copy header
//! [20-..] MessageBody      = body bytes (type-dependent)
//! ```

use crate::header::MessageHeader;

/// Size of the frame length prefix in bytes
pub const FRAME_PREFIX_SIZE: usize = 4;

/// Read a framed NWP message from a byte stream.
/// Returns (bytes_read, message_bytes_slice).
///
/// This is the synchronous (blocking) version. For async, see the `network_async` module.
pub fn read_message_blocking(
    stream: &mut impl std::io::Read,
    buf: &mut Vec<u8>,
) -> std::io::Result<usize> {
    // Read 4-byte frame length
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let frame_len = u32::from_le_bytes(len_buf) as usize;

    // Verify frame is within reasonable bounds
    if frame_len > crate::MAX_MESSAGE_SIZE as usize || frame_len < MessageHeader::SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid frame length: {}", frame_len),
        ));
    }

    // Ensure buffer is large enough
    buf.resize(frame_len, 0);

    // Read the exact frame
    stream.read_exact(buf)?;

    Ok(frame_len)
}

/// Write a framed NWP message to a byte stream.
/// Prepends the 4-byte frame length.
pub fn write_message_blocking(
    stream: &mut impl std::io::Write,
    msg_bytes: &[u8],
) -> std::io::Result<()> {
    // Write 4-byte frame length
    let len = msg_bytes.len() as u32;
    stream.write_all(&len.to_le_bytes())?;

    // Write the message itself
    stream.write_all(msg_bytes)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::MessageHeader;
    use crate::types::MessageType;

    #[test]
    fn test_frame_roundtrip() {
        let mut buffer = Vec::new();

        // Create a PING message
        let header = MessageHeader::new(MessageType::Ping as u8, 0, 0);
        let msg_bytes = header.as_bytes();

        // Write to buffer
        write_message_blocking(&mut buffer, msg_bytes).unwrap();

        // Read back from buffer
        let mut read_buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&buffer);
        let len = read_message_blocking(&mut cursor, &mut read_buf).unwrap();

        assert_eq!(len, MessageHeader::SIZE);
        assert_eq!(&read_buf, msg_bytes);
    }
}
