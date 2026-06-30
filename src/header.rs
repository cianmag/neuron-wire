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
//!
//! # Security Flags
//!
//! The `flags` field in MessageHeader indicates additional processing:
//! - `ENCRYPTED` (0x01): payload is AEAD-encrypted with XChaCha20-Poly1305
//! - `AUTHENTICATED` (0x02): payload prepended with auth prefix (pubkey + signature)
//! - `HANDSHAKE` (0x04): this message is part of a secure channel handshake
//! - `AUDIT_REQUEST` (0x08): sender requests audit proof
//! - `BOOTSTRAP` (0x10): payload is a bootstrap proof

use crate::{HEADER_SIZE, MAGIC, MAX_BODY_SIZE, VERSION};

/// Payload is AEAD-encrypted with XChaCha20-Poly1305.
pub const FLAG_ENCRYPTED: u16 = 0x0001;
/// Payload includes identity auth prefix (32-byte public key + 64-byte signature).
pub const FLAG_AUTHENTICATED: u16 = 0x0002;
/// This message is part of a secure channel handshake.
pub const FLAG_HANDSHAKE: u16 = 0x0004;
/// Sender is requesting an audit proof in response.
pub const FLAG_AUDIT_REQUEST: u16 = 0x0008;
/// Message body is a bootstrap proof payload.
pub const FLAG_BOOTSTRAP: u16 = 0x0010;

/// 16-byte message header — zero-copy accessible via repr(C)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MessageHeader {
    /// 4-byte protocol magic (`NWP\0`) — validated on deserialization.
    pub magic: [u8; 4],
    /// Wire-format version — must match [`VERSION`].
    pub version: u8,
    /// Message-type discriminator (e.g. 0 = Ping, 1 = Pong).
    pub msg_type: u8,
    /// Bit-flag field for control flags.
    pub flags: u16,
    /// Length of the message body (payload) in bytes.
    pub body_len: u32,
    /// CRC32 of header bytes `[0..12)` — covers magic through body_len.
    pub header_crc: u32,
}

impl MessageHeader {
    /// Construct a new `MessageHeader` with the given fields.
    ///
    /// Automatically computes and sets `header_crc` from the other fields.
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

    /// Validate the header: checks magic, version, CRC, and body size.
    ///
    /// Returns `Ok(())` on success, or a [`HeaderError`] describing the
    /// first validation failure encountered.
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

    /// Total on-wire size: header size plus body length.
    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.body_len as usize
    }
}

/// Read the header from a buffer (zero-copy) without ownership
pub fn read_header(buf: &[u8]) -> Result<&MessageHeader, HeaderError> {
    MessageHeader::from_bytes(buf)
}

/// Errors that can occur when parsing or validating a [`MessageHeader`].
#[derive(Debug)]
pub enum HeaderError {
    /// The input buffer is shorter than [`HEADER_SIZE`].
    ShortBuffer(usize),
    /// The magic bytes do not match the expected protocol magic.
    BadMagic([u8; 4]),
    /// The wire-format version does not match the expected [`VERSION`].
    BadVersion(u8),
    /// The header CRC does not match the computed CRC of bytes `[0..12)`.
    BadCrc,
    /// The declared body length exceeds [`MAX_BODY_SIZE`].
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

/// Build a Ping frame (msg_type = 0) with an empty body.
pub fn build_ping() -> Vec<u8> {
    build_frame(0, Vec::new(), 0)
}

/// Build a Pong frame (msg_type = 1) with an empty body.
pub fn build_pong() -> Vec<u8> {
    build_frame(1, Vec::new(), 0)
}

/// Parse a complete frame: returns (header, body_slice)
pub fn parse_frame(buf: &[u8]) -> Result<(&MessageHeader, &[u8]), HeaderError> {
    let header = MessageHeader::from_bytes(buf)?;
    let body = &buf[HEADER_SIZE..header.total_size()];
    Ok((header, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HEADER_SIZE, MAGIC, MAX_BODY_SIZE, VERSION};

    #[test]
    fn test_header_new_validates_crc() {
        let h = MessageHeader::new(5, 100, 0x0001);
        assert_eq!(h.magic, MAGIC);
        assert_eq!(h.version, VERSION);
        assert_eq!(h.msg_type, 5);
        assert_eq!(h.body_len, 100);
        assert_eq!(h.flags, 0x0001);
        assert!(h.verify_crc());
    }

    #[test]
    fn test_header_from_bytes_valid() {
        let h = MessageHeader::new(2, 64, 0);
        let bytes = h.to_bytes();
        let parsed = MessageHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.msg_type, 2);
        assert_eq!(parsed.body_len, 64);
    }

    #[test]
    fn test_header_short_buffer() {
        let buf = [0u8; 4];
        let err = MessageHeader::from_bytes(&buf).unwrap_err();
        match err {
            HeaderError::ShortBuffer(n) => assert_eq!(n, 4),
            _ => panic!("expected ShortBuffer"),
        }
    }

    #[test]
    fn test_header_bad_magic() {
        let mut h = MessageHeader::new(0, 0, 0);
        h.magic = [0; 4]; // corrupt magic
        let bytes = h.to_bytes();
        let err = MessageHeader::from_bytes(&bytes).unwrap_err();
        match err {
            HeaderError::BadMagic(m) => assert_eq!(m, [0; 4]),
            _ => panic!("expected BadMagic"),
        }
    }

    #[test]
    fn test_header_bad_version() {
        let mut h = MessageHeader::new(0, 0, 0);
        h.version = 99;
        let bytes = h.to_bytes();
        let err = MessageHeader::from_bytes(&bytes).unwrap_err();
        match err {
            HeaderError::BadVersion(v) => assert_eq!(v, 99),
            _ => panic!("expected BadVersion"),
        }
    }

    #[test]
    fn test_header_bad_crc() {
        let mut h = MessageHeader::new(0, 0, 0);
        h.header_crc = 0xDEADBEEF; // corrupt CRC
        let bytes = h.to_bytes();
        let err = MessageHeader::from_bytes(&bytes).unwrap_err();
        match err {
            HeaderError::BadCrc => {}
            _ => panic!("expected BadCrc"),
        }
    }

    #[test]
    fn test_header_body_too_large() {
        let mut h = MessageHeader::new(0, 0, 0);
        h.body_len = MAX_BODY_SIZE + 1;
        h.header_crc = h.compute_crc(); // recompute with modified body_len
        let bytes = h.to_bytes();
        let err = MessageHeader::from_bytes(&bytes).unwrap_err();
        match err {
            HeaderError::BodyTooLarge(s) => assert!(s > MAX_BODY_SIZE),
            _ => panic!("expected BodyTooLarge"),
        }
    }

    #[test]
    fn test_build_frame_and_parse() {
        let body = vec![0xAB, 0xCD, 0xEF];
        let frame = build_frame(3, body.clone(), 0);
        // frame starts with 4-byte length prefix
        let frame_len = u32::from_le_bytes(frame[0..4].try_into().unwrap()) as usize;
        assert_eq!(frame_len, frame.len());
        // parse the header+body portion (after 4-byte len)
        let (header, parsed_body) = parse_frame(&frame[4..]).unwrap();
        assert_eq!(header.msg_type, 3);
        assert_eq!(parsed_body, &body[..]);
    }

    #[test]
    fn test_header_total_size() {
        let h = MessageHeader::new(0, 256, 0);
        assert_eq!(h.total_size(), HEADER_SIZE + 256);
    }

    #[test]
    fn test_build_ping_pong() {
        let ping = build_ping();
        let pong = build_pong();
        assert!(ping.len() == 4 + HEADER_SIZE); // 4-byte len + header
        assert!(pong.len() == 4 + HEADER_SIZE);
        // msg_type 0 = Ping, 1 = Pong
        let ping_h = MessageHeader::from_bytes(&ping[4..]).unwrap();
        assert_eq!(ping_h.msg_type, 0);
        let pong_h = MessageHeader::from_bytes(&pong[4..]).unwrap();
        assert_eq!(pong_h.msg_type, 1);
    }

    #[test]
    fn test_header_error_display() {
        let e = HeaderError::ShortBuffer(4);
        let s = format!("{}", e);
        assert!(s.contains("4") && s.contains("16"));
        let e = HeaderError::BadMagic([0; 4]);
        assert!(format!("{}", e).contains("magic"));
        let e = HeaderError::BadVersion(99);
        assert!(format!("{}", e).contains("99"));
        let e = HeaderError::BadCrc;
        assert!(format!("{}", e).contains("CRC"));
    }
}
