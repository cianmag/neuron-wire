//! CRC32 helper
use crc32fast::Hasher;

#[inline]
pub fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}
