//! MessageHeader — 16 bytes, every message starts here.
//!
//! Binary layout (little-endian):
//! ```text
//! [0-3]   magic: [u8; 4]    = "NWP\0"
//! [4]     version: u8       = 2
//! [5]     msg_type: u8      = MsgType discriminant
//! [6-7]   flags: u16        = bit flags
//! [8-11]  body_len: u32     = body length in bytes
//! [12-15] header_crc: u32   = CRC32 of bytes [0..12)
//! ```

use crate::{HEADER_SIZE, MAGIC, MAX_BODY_SIZE, VERSION};

/// 16-byte message header — zero-copy accessible via repr(C)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MessageHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub msg_type: u8,
    pub flags: u16,
    pub body_len: u32,
    pub header_crc: u32,
}

impl MessageHeader {
    pub fn new(msg_type: u8, body_len: u32, flags: u16) -> Self {
        let mut h = MessageHeader {
            magic: MAGIC,
            version: VERSION,
            msg_type,
            flags,
            body_len,
            header_crc: 0,
        };
        h.header_crc = h.compute_crc();
        h
    }

    /// Read from a byte slice (zero-copy cast)
    pub fn from_bytes(buf: &[u8]) -> Result<&Self, HeaderError> {
        if buf.len() < HEADER_SIZE {
            return Err(HeaderError::ShortBuffer(buf.len()));
        }
        let h = unsafe { &*(buf.as_ptr() as *const MessageHeader) };
        h.validate()?;
        Ok(h)
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; 16] {
        unsafe { *(self as *const MessageHeader as *const [u8; 16]) }
    }

    fn compute_crc(&self) -> u32 {
        let bytes = self.to_bytes();
        crate::crc::crc32(&bytes[..12])
    }

    fn verify_crc(&self) -> bool {
        self.header_crc == self.compute_crc()
    }

    pub fn validate(&self) -> Result<(), HeaderError> {
        if self.magic != MAGIC {
            return Err(HeaderError::BadMagic(self.magic));
        }
        if self.version != VERSION {
            return Err(HeaderError::BadVersion(self.version));
        }
        if !self.verify_crc() {
            return Err(HeaderError::BadCrc);
        }
        if self.body_len > MAX_BODY_SIZE {
            return Err(HeaderError::BodyTooLarge(self.body_len));
        }
        Ok(())
    }

    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.body_len as usize
    }
}

/// Read the header from a buffer (zero-copy) without ownership
pub fn read_header(buf: &[u8]) -> Result<&MessageHeader, HeaderError> {
    MessageHeader::from_bytes(buf)
}

#[derive(Debug)]
pub enum HeaderError {
    ShortBuffer(usize),
    BadMagic([u8; 4]),
    BadVersion(u8),
    BadCrc,
    BodyTooLarge(u32),
}

impl core::fmt::Display for HeaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HeaderError::ShortBuffer(n) => write!(f, "buffer too short: {} < {}", n, HEADER_SIZE),
            HeaderError::BadMagic(m) => write!(f, "bad magic: {:02x?}", m),
            HeaderError::BadVersion(v) => write!(f, "bad version: {}", v),
            HeaderError::BadCrc => write!(f, "CRC mismatch"),
            HeaderError::BodyTooLarge(s) => write!(f, "body too large: {}", s),
        }
    }
}

// ─── Helper to build a complete framed message ──────────────────

/// Build a complete wire frame: `[4-byte len][header][body]`
pub fn build_frame(msg_type: u8, body: Vec<u8>, flags: u16) -> Vec<u8> {
    let body_len = body.len() as u32;
    let header = MessageHeader::new(msg_type, body_len, flags);
    let total = 4 + HEADER_SIZE + body.len();
    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&(total as u32).to_le_bytes()); // frame length (EXCLUDING this field)
    frame.extend_from_slice(&header.to_bytes());
    frame.extend_from_slice(&body);
    frame
}

pub fn build_ping() -> Vec<u8> {
    build_frame(0, Vec::new(), 0)
}

pub fn build_pong() -> Vec<u8> {
    build_frame(1, Vec::new(), 0)
}

/// Parse a complete frame: returns (header, body_slice)
pub fn parse_frame(buf: &[u8]) -> Result<(&MessageHeader, &[u8]), HeaderError> {
    let header = MessageHeader::from_bytes(buf)?;
    let body = &buf[HEADER_SIZE..header.total_size()];
    Ok((header, body))
}
