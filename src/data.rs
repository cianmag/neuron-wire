//! Data message — bulk payload transfer (lazy, zero-copy).
//!
//! Data is not sent inline with spikes/commands. Instead, it's sent separately
//! when explicitly requested. The DataHeader is fixed-size (24 bytes) and the
//! payload follows immediately after in the buffer.

use crate::NeuronId;

/// Data message header — 24 bytes, fixed-size.
///
/// Binary layout:
/// ```text
/// [0-7]   sender_id: u64       = sender neuron ID
/// [8-11]  data_hash: u32       = CRC32 of the payload
/// [12-13] content_type: u16    = type of data
/// [14-15] compression: u16     = compression codec
/// [16-19] original_len: u32    = original uncompressed size
/// [20-23] payload_len: u32     = actual payload size
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DataHeader {
    pub sender_id: NeuronId,
    pub data_hash: u32,
    pub content_type: u16,
    pub compression: u16,
    pub original_len: u32,
    pub payload_len: u32,
}

impl DataHeader {
    pub const SIZE: usize = 24;

    pub fn new(
        sender_id: NeuronId,
        content_type: u16,
        compression: u16,
        payload: &[u8],
    ) -> Self {
        let payload_len = payload.len() as u32;
        let data_hash = crate::crc::crc32(payload);
        DataHeader {
            sender_id,
            data_hash,
            content_type,
            compression,
            original_len: payload_len,
            payload_len,
        }
    }

    /// Validate the CRC against the actual payload bytes
    #[inline]
    pub fn validate_payload(&self, payload: &[u8]) -> bool {
        let computed = crate::crc::crc32(payload);
        computed == self.data_hash
    }

    /// Zero-copy: interpret bytes as a DataHeader
    #[inline]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &DataHeader {
        assert!(bytes.len() >= Self::SIZE);
        &*(bytes.as_ptr() as *const DataHeader)
    }
}
