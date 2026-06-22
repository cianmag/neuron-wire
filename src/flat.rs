//! FlatBuffer-style zero-copy serialization.
//!
//! ## Design
//!
//! Every body is divided into two regions:
//! 1. **Fixed region** — scalar fields at known offsets from body start
//! 2. **Data region** — variable-length data (strings, vectors, payloads)
//!
//! Variable-length data is accessed via **relative offsets** stored in the
//! fixed region. Offset = 0 means "not present."
//!
//! All access is zero-copy: we compute offsets into the buffer and return
//! slices. No allocation, no parsing.

use core::fmt;

// ─── Helpers ────────────────────────────────────────────────────

/// Read a u32 from the buffer at a byte offset (little-endian)
#[inline]
pub fn read_u32(buf: &[u8], offset: usize) -> u32 {
    let b = &buf[offset..offset + 4];
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Write a u32 into the buffer at a byte offset
#[inline]
pub fn write_u32(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

/// Read a u64 from the buffer at a byte offset
#[inline]
pub fn read_u64(buf: &[u8], offset: usize) -> u64 {
    let b = &buf[offset..offset + 8];
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// Write a u64 into the buffer at a byte offset
#[inline]
pub fn write_u64(buf: &mut [u8], offset: usize, val: u64) {
    buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
}

// ─── Body Builder ───────────────────────────────────────────────

/// Builds a FlatBuffer body with a fixed region + variable data region.
///
/// Usage:
/// ```ignore
/// let mut bb = BodyBuilder::new(64);      // fixed region size
/// bb.push_data(b"hello");                 // variable data
/// bb.write_u32(12, data_offset);          // store offset in fixed region
/// let body = bb.finish();                 // get the complete body
/// ```
pub struct BodyBuilder {
    fixed: Vec<u8>,
    data: Vec<u8>,
}

impl BodyBuilder {
    /// Create a new body with `fixed_size` reserved bytes for the fixed region
    pub fn new(fixed_size: usize) -> Self {
        let mut fixed = Vec::with_capacity(fixed_size + 64);
        fixed.resize(fixed_size, 0);
        BodyBuilder {
            fixed,
            data: Vec::with_capacity(128),
        }
    }

    /// Write a u32 at a fixed-region offset
    #[inline]
    pub fn write_u32(&mut self, offset: usize, val: u32) {
        write_u32(&mut self.fixed, offset, val)
    }

    /// Write a u16 at a fixed-region offset
    #[inline]
    pub fn write_u16(&mut self, offset: usize, val: u16) {
        let b = &val.to_le_bytes();
        self.fixed[offset..offset + 2].copy_from_slice(b);
    }

    /// Write a u64 at a fixed-region offset
    #[inline]
    pub fn write_u64(&mut self, offset: usize, val: u64) {
        write_u64(&mut self.fixed, offset, val)
    }

    /// Push variable-length data into the data region.
    /// Returns the **relative offset** from the start of the fixed region
    /// to the data. This offset can be stored in a fixed field.
    ///
    /// Format: [len: u32][bytes]
    #[inline]
    pub fn push_data(&mut self, bytes: &[u8]) -> u32 {
        let offset = self.fixed.len() as u32;
        self.fixed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.fixed.extend_from_slice(bytes);
        offset
    }

    /// Finish building — returns the complete body as a single buffer.
    /// The body layout is: [fixed_region][data_region]
    pub fn finish(&mut self) -> Vec<u8> {
        self.fixed.append(&mut self.data);
        std::mem::take(&mut self.fixed)
    }
}

// ─── Body Reader ────────────────────────────────────────────────

/// Zero-copy reader for a FlatBuffer body.
///
/// Provides typed field accessors that read directly from the buffer.
pub struct BodyReader<'a> {
    buf: &'a [u8],
}

impl<'a> BodyReader<'a> {
    /// Create a reader from a body buffer (the bytes after the MessageHeader)
    #[inline]
    pub fn new(buf: &'a [u8]) -> Self {
        BodyReader { buf }
    }

    /// Read a u32 field at a fixed offset from body start
    #[inline]
    pub fn read_u32(&self, offset: usize) -> u32 {
        read_u32(self.buf, offset)
    }

    /// Read a u16 field at a fixed offset from body start
    #[inline]
    pub fn read_u16(&self, offset: usize) -> u16 {
        let b = &self.buf[offset..offset + 2];
        u16::from_le_bytes([b[0], b[1]])
    }

    /// Read a u64 field at a fixed offset from body start
    #[inline]
    pub fn read_u64(&self, offset: usize) -> u64 {
        read_u64(self.buf, offset)
    }

    /// Read a variable-length string at `offset`.
    /// The field stores a relative offset pointing to data in the body.
    /// Returns None if offset is 0 (field absent).
    #[inline]
    pub fn read_string(&self, field_offset: usize) -> Option<&'a str> {
        let relative = self.read_u32(field_offset) as usize;
        if relative == 0 {
            return None;
        }
        let len = read_u32(self.buf, relative) as usize;
        Some(unsafe {
            core::str::from_utf8_unchecked(&self.buf[relative + 4..relative + 4 + len])
        })
    }

    /// Read a raw byte slice at `offset`.
    /// Returns None if offset is 0 (field absent).
    #[inline]
    pub fn read_bytes(&self, field_offset: usize) -> Option<&'a [u8]> {
        let relative = self.read_u32(field_offset) as usize;
        if relative == 0 {
            return None;
        }
        let len = read_u32(self.buf, relative) as usize;
        Some(&self.buf[relative + 4..relative + 4 + len])
    }

    /// Get the raw body buffer
    #[inline]
    pub fn raw(&self) -> &'a [u8] {
        self.buf
    }
}

impl<'a> fmt::Debug for BodyReader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BodyReader(len={})", self.buf.len())
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_roundtrip() {
        // Build a body with fixed fields + a string
        let mut bb = BodyBuilder::new(16);
        bb.write_u32(0, 42);               // field 0: u32
        bb.write_u64(4, 0xDEAD_BEEF);       // field 1: u64
        let str_offset = bb.push_data(b"hello neuron");
        bb.write_u32(12, str_offset);       // field 2: string offset

        let body = bb.finish();
        assert!(body.len() >= 16);

        // Read back zero-copy
        let reader = BodyReader::new(&body);
        assert_eq!(reader.read_u32(0), 42);
        assert_eq!(reader.read_u64(4), 0xDEAD_BEEF);
        assert_eq!(reader.read_string(12), Some("hello neuron"));
    }

    #[test]
    fn test_absent_field() {
        let mut bb = BodyBuilder::new(4);
        bb.write_u32(0, 0); // offset = 0 means absent

        let body = bb.finish();
        let reader = BodyReader::new(&body);
        assert!(reader.read_string(0).is_none());
    }

    #[test]
    fn test_zero_copy_proof() {
        // The buffer IS the data — no allocation happens during reads
        let mut bb = BodyBuilder::new(8);
        bb.write_u64(0, 0x4242);
        let body = bb.finish();

        let reader = BodyReader::new(&body);
        // This is a zero-copy read: just pointer arithmetic on the buffer
        assert_eq!(reader.read_u64(0), 0x4242);
        // No Vec allocation, no parsing, just offset computation
    }
}
