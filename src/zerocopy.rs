//! Zero-copy helpers — safely reinterpret byte buffers as message types.
//!
//! The core of the "Extremely Lite Language": bytes on the wire ARE the data
//! structure. No parsing, no deserialization, no memory allocation.
//!
//! ## How it works
//!
//! 1. Receive bytes from network into a buffer
//! 2. Call `MessageHeader::from_bytes(buf)` — zero-copy cast
//! 3. Validate the header (magic, CRC)
//! 4. Read body bytes from `HEADER_SIZE..total_size`
//! 5. Read fields directly from the body slice
//! 6. When done, the buffer is returned to the pool
//!
//! No memory was allocated. No data was copied. The entire message was
//! "deserialized" by reading bytes that were already in memory.

use crate::header::{HeaderError, MessageHeader};
use crate::types::MsgType;
use crate::HEADER_SIZE;

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
    /// Zero-copy — the returned references point directly into `buf`.
    ///
    /// Returns `Err(HeaderError)` if the buffer is too small or invalid.
    #[inline]
    pub fn parse(buf: &'a [u8]) -> Result<MessageRef<'a>, HeaderError> {
        let header = MessageHeader::from_bytes(buf)?;
        let total_size = header.total_size();
        if buf.len() < total_size {
            return Err(HeaderError::ShortBuffer(buf.len()));
        }
        let body = &buf[HEADER_SIZE..total_size];
        Ok(MessageRef { header, body })
    }

    /// Get the message type
    #[inline]
    pub fn msg_type(&self) -> Option<MsgType> {
        MsgType::from_u8(self.header.msg_type)
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
        let header = MessageHeader::new(MsgType::Ping as u8, 0, 0);
        let mut buf = Vec::with_capacity(HEADER_SIZE);
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
        let header = MessageHeader::new(MsgType::Pong as u8, 0, 0);
        let mut buf = Vec::with_capacity(HEADER_SIZE);
        buf.extend_from_slice(&header.magic);
        buf.push(header.version);
        buf.push(header.msg_type);
        buf.extend_from_slice(&header.flags.to_le_bytes());
        buf.extend_from_slice(&header.body_len.to_le_bytes());
        buf.extend_from_slice(&header.header_crc.to_le_bytes());
        buf
    }
}

/// Pre-allocated buffer pool for zero-copy message I/O.
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
        self.pool
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.buffer_size))
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
    use crate::types;

    #[test]
    fn test_parse_ping() {
        let buf = MessageRef::new_ping();
        let msg = MessageRef::parse(&buf).unwrap();
        assert_eq!(msg.msg_type(), Some(MsgType::Ping));
        assert_eq!(msg.total_size(), HEADER_SIZE);
    }

    #[test]
    fn test_parse_empty() {
        let result = MessageRef::parse(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_too_short() {
        let buf = vec![0u8; 4];
        let result = MessageRef::parse(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new(65535);
        let buf = pool.acquire();
        assert!(buf.capacity() >= 65535);
        pool.release(buf);
        let buf2 = pool.acquire();
        assert!(buf2.is_empty());
        assert!(buf2.capacity() >= 65535);
    }

    #[test]
    fn test_has_flag() {
        let buf = MessageRef::new_ping();
        let msg = MessageRef::parse(&buf).unwrap();
        assert!(!msg.has_flag(types::flags::COMPRESSED));
    }
}
