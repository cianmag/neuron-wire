//! CRC32 helper
use crc32fast::Hasher;

/// Compute a CRC-32/ISO-HDLC checksum over `data`.
///
/// Uses the `crc32fast` crate for a hardware-accelerated implementation.
/// Returns the CRC-32 checksum (ISO-HDLC / PKZIP variant).
#[inline]
#[inline]
pub fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_empty() {
        // CRC32 of empty is a well-known constant: 0x00000000
        assert_eq!(crc32(&[]), 0x00000000);
    }

    #[test]
    fn test_crc32_known_value() {
        // CRC32 of b"hello" = 0x3610A686 (CRC-32/ISO-HDLC)
        assert_eq!(crc32(b"hello"), 0x3610A686);
    }

    #[test]
    fn test_crc32_different_inputs() {
        assert_ne!(crc32(b"abc"), crc32(b"xyz"));
    }

    #[test]
    fn test_crc32_deterministic() {
        let data = b"the quick brown fox jumps over the lazy dog";
        assert_eq!(crc32(data), crc32(data));
    }

    #[test]
    fn test_crc32_nonzero_different() {
        let a = crc32(&[1, 2, 3]);
        let b = crc32(&[4, 5, 6]);
        assert_ne!(a, b, "different inputs should yield different CRCs");
    }
}
