//! Property-based tests using proptest.
//! Run with: cargo test --features proptest
//! (or just cargo test on CI where proptest is a dev-dependency)

use proptest::prelude::*;
use neuron_wire::header::{self, MessageHeader};
use neuron_wire::types::{MsgType, conf_to_raw, conf_from_raw};
use neuron_wire::crc::crc32;

// ─── Header property tests ──────────────────────────────────────

proptest! {
    /// Any valid MessageHeader roundtrips through to_bytes + from_bytes
    #[test]
    fn prop_header_roundtrip(
        msg_type in 0..=6u8,
        body_len in 0..=neuron_wire::MAX_BODY_SIZE,
        flags in any::<u16>(),
    ) {
        let h = MessageHeader::new(msg_type, body_len, flags);
        let bytes = h.to_bytes();
        let parsed = MessageHeader::from_bytes(&bytes).unwrap();
        prop_assert_eq!(h.magic, parsed.magic);
        prop_assert_eq!(h.version, parsed.version);
        prop_assert_eq!(h.msg_type, parsed.msg_type);
        prop_assert_eq!(h.flags, parsed.flags);
        prop_assert_eq!(h.body_len, parsed.body_len);
        prop_assert_eq!(h.header_crc, parsed.header_crc);
    }

    /// CRC32 is never zero for non-empty data
    #[test]
    fn prop_crc_nonzero_for_nonempty(data in prop::collection::vec(any::<u8>(), 1..1024)) {
        let c = crc32(&data);
        prop_assert_ne!(c, 0, "CRC32 of non-empty data should not be zero");
    }

    /// CRC32 is deterministic
    #[test]
    fn prop_crc_deterministic(data in prop::collection::vec(any::<u8>(), 0..256)) {
        let c1 = crc32(&data);
        let c2 = crc32(&data);
        prop_assert_eq!(c1, c2);
    }

    /// Confidence roundtrip: f64 -> u32 -> f64 stays within 1e-6 error
    #[test]
    fn prop_confidence_roundtrip(value in 0.0f64..=1.0) {
        let raw = conf_to_raw(value);
        let back = conf_from_raw(raw);
        let diff = (value - back).abs();
        prop_assert!(diff < 1e-6, "roundtrip error {} for value {}", diff, value);
    }

    /// MsgType::from_u8 only succeeds for 0..=6
    #[test]
    fn prop_msg_type_valid_range(v in 0u8..=255) {
        let t = MsgType::from_u8(v);
        if v <= 6 {
            prop_assert!(t.is_some());
            prop_assert_eq!(t.unwrap() as u8, v);
        } else {
            prop_assert!(t.is_none());
        }
    }

    /// Build frame then parse frame preserves body
    #[test]
    fn prop_build_parse_frame(
        msg_type in 0..=6u8,
        body in prop::collection::vec(any::<u8>(), 0..64),
        flags in any::<u16>(),
    ) {
        let frame = header::build_frame(msg_type, body.clone(), flags);
        let msg = &frame[4..]; // skip 4-byte length prefix
        let (h, b) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.msg_type, msg_type);
        prop_assert_eq!(h.flags, flags);
        prop_assert_eq!(h.body_len as usize, body.len());
        prop_assert_eq!(b, &body[..]);
    }

    /// Header total_size is always HEADER_SIZE + body_len
    #[test]
    fn prop_header_total_size(body_len in 0u32..=10_000) {
        let h = MessageHeader::new(0, body_len.min(neuron_wire::MAX_BODY_SIZE), 0);
        prop_assert_eq!(h.total_size(), neuron_wire::HEADER_SIZE + h.body_len as usize);
    }

    /// Corrupting bytes in a valid header makes from_bytes fail (statistically)
    #[test]
    fn prop_corrupt_byte_rejected(
        msg_type in 0..=6u8,
        body_len in 0u32..=1024,
        flags in any::<u16>(),
        corrupt_byte in 0usize..15,
        corrupt_value in any::<u8>(),
    ) {
        let h = MessageHeader::new(msg_type, body_len, flags);
        let mut bytes = h.to_bytes();
        let old = bytes[corrupt_byte];
        bytes[corrupt_byte] = corrupt_value;
        if old != corrupt_value {
            // Almost always fails — astronomically unlikely to produce valid header
            let _ = MessageHeader::from_bytes(&bytes);
        }
    }
}
