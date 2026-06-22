//! Zero-copy helpers — safely reinterpret byte buffers as message types.
//!
//! The core of the "Extremely Lite Language": bytes on the wire ARE the data
//! structure. No parsing, no deserialization, no memory allocation.
//!
//! ## How it works
//!
//! 1. Receive bytes from network into a buffer
//! 2. Cast the buffer pointer to a `MessageHeader` — reads the first 16 bytes
//! 3. Validate the header (magic, CRC)
//! 4. Cast the buffer + 16 to the appropriate body type
//! 5. Read fields directly from the buffer
//! 6. When done, the buffer is returned to the pool
//!
//! No memory was allocated. No data was copied. The entire message was
//! "deserialized" by reading bytes that were already in memory.

use crate::header::MessageHeader;
use crate::types::MessageType;

/// A parsed message referencing a network buffer.
/// This is zero-copy — it borrows the buffer, it doesn't own it.
#[derive(Debug)]
pub struct MessageRef<'a> {
    /// Reference to the header (points into the buffer)
    pub header: &'a MessageHeader,
    /// Reference to the body bytes (points into the buffer, right after header)
    pub body: &'a [u8],
}

impl<'a> MessageRef<'a> {
    /// Parse a message from a byte buffer.
    /// This is zero-copy — the returned references point directly into `buf`.
    ///
    /// Returns None if the buffer is too small or the header is invalid.
    #[inline]
    pub fn parse(buf: &'a [u8]) -> Option<MessageRef<'a>> {
        if buf.len() < MessageHeader::SIZE {
            return None;
        }

        // Zero-copy: cast the buffer to a header
        let header = unsafe { MessageHeader::from_bytes(buf) };

        // Validate the header
        if header.validate().is_err() {
            return None;
        }

        let total_size = header.total_size();
        if buf.len() < total_size {
            return None;
        }

        let body = &buf[MessageHeader::SIZE..total_size];

        Some(MessageRef { header, body })
    }

    /// Get the message type
    #[inline]
    pub fn msg_type(&self) -> MessageType {
        MessageType::from_u8(self.header.msg_type).unwrap_or(MessageType::Reserved)
    }

    /// Get flags
    #[inline]
    pub fn flags(&self) -> u16 {
        self.header.flags
    }

    /// Check if a specific flag is set
    #[inline]
    pub fn has_flag(&self, flag: u16) -> bool {
        self.header.flags & flag != 0
    }

    /// Get the total message size (header + body)
    #[inline]
    pub fn total_size(&self) -> usize {
        self.header.total_size()
    }

    /// Create a buffer with a Ping message
    pub fn new_ping() -> Vec<u8> {
        let header = MessageHeader::new(MessageType::Ping as u8, 0, 0);
        let mut buf = Vec::with_capacity(MessageHeader::SIZE);
        buf.extend_from_slice(&header.magic);
        buf.push(header.version);
        buf.push(header.msg_type);
        buf.extend_from_slice(&header.flags.to_le_bytes());
        buf.extend_from_slice(&header.body_len.to_le_bytes());
        buf.extend_from_slice(&header.header_crc.to_le_bytes());
        buf
    }

    /// Create a buffer with a Pong message
    pub fn new_pong() -> Vec<u8> {
        let header = MessageHeader::new(MessageType::Pong as u8, 0, 0);
        let mut buf = Vec::with_capacity(MessageHeader::SIZE);
        buf.extend_from_slice(&header.magic);
        buf.push(header.version);
        buf.push(header.msg_type);
        buf.extend_from_slice(&header.flags.to_le_bytes());
        buf.extend_from_slice(&header.body_len.to_le_bytes());
        buf.extend_from_slice(&header.header_crc.to_le_bytes());
        buf
    }
}

/// Pre-allocated buffer pool for zero-copy message I/O
/// Reuses buffers to avoid allocation during high-throughput messaging.
pub struct BufferPool {
    pool: Vec<Vec<u8>>,
    buffer_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool with the given buffer size
    pub fn new(buffer_size: usize) -> Self {
        BufferPool {
            pool: Vec::new(),
            buffer_size,
        }
    }

    /// Get a buffer from the pool (or allocate a new one)
    #[inline]
    pub fn acquire(&mut self) -> Vec<u8> {
        self.pool.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_size))
    }

    /// Return a buffer to the pool for reuse
    #[inline]
    pub fn release(&mut self, mut buf: Vec<u8>) {
        buf.clear();
        if self.pool.len() < 100 {
            self.pool.push(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandBody;

    #[test]
    fn test_parse_ping() {
        let buf = MessageRef::new_ping();
        let msg = MessageRef::parse(&buf).unwrap();
        assert_eq!(msg.msg_type(), MessageType::Ping);
        assert_eq!(msg.total_size(), MessageHeader::SIZE);
    }

    #[test]
    fn test_parse_command() {
        let cmd = CommandBody::new(1, 2, 0.95, 0xABCD, 100_000, 0x42, 0xFF);
        let header = MessageHeader::new(MessageType::Command as u8, CommandBody::SIZE as u32, 0);

        let mut buf = Vec::with_capacity(header.total_size());
        buf.extend_from_slice(&header.magic);
        buf.push(header.version);
        buf.push(header.msg_type);
        buf.extend_from_slice(&header.flags.to_le_bytes());
        buf.extend_from_slice(&header.body_len.to_le_bytes());
        buf.extend_from_slice(&header.header_crc.to_le_bytes());
        // body
        buf.extend_from_slice(&cmd.command_id.to_le_bytes());
        buf.extend_from_slice(&cmd.prediction_code.to_le_bytes());
        buf.extend_from_slice(&cmd.confidence_raw.to_le_bytes());
        buf.extend_from_slice(&cmd.context_hash.to_le_bytes());
        buf.extend_from_slice(&cmd.deadline_us.to_le_bytes());
        buf.extend_from_slice(&cmd.source_id.to_le_bytes());
        buf.extend_from_slice(&cmd.target_mask.to_le_bytes());

        let msg = MessageRef::parse(&buf).unwrap();
        assert_eq!(msg.msg_type(), MessageType::Command);

        // Zero-copy: cast body bytes directly to CommandBody
        let parsed_cmd = unsafe { CommandBody::from_bytes(msg.body) };
        assert_eq!(parsed_cmd.command_id, 1);
        assert_eq!(parsed_cmd.prediction_code, 2);
        assert!((parsed_cmd.confidence() - 0.95).abs() < 0.001);
    }
}
