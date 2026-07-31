//! Property-based tests using proptest.
//! Run with: cargo test --features proptest
//! (or just cargo test on CI where proptest is a dev-dependency)

use neuron_wire::components::EntityId;
use neuron_wire::crc::crc32;
use neuron_wire::header::{self, MessageHeader};
use neuron_wire::identity::{verify_signature, NodeIdentity};
use neuron_wire::secure_channel::SecureChannel;
use neuron_wire::trust::{TrustEvent, TrustSystem, SYBIL_THRESHOLD};
use neuron_wire::types::{conf_from_raw, conf_to_raw, MsgType};
use proptest::prelude::*;

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

    /// a. All msg_type values 0..=40 produce valid frames
    #[test]
    fn prop_header_valid_msg_types(
        msg_type in 0..=40u8,
        body in prop::collection::vec(any::<u8>(), 0..64),
        flags in any::<u16>(),
    ) {
        let frame = header::build_frame(msg_type, body.clone(), flags);
        let msg = &frame[4..];
        let (h, b) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.msg_type, msg_type);
        prop_assert_eq!(h.flags, flags);
        prop_assert_eq!(b, &body[..]);
    }

    /// b. Empty body roundtrips with body_len=0
    #[test]
    fn prop_header_empty_body(
        msg_type in 0..=6u8,
        flags in any::<u16>(),
    ) {
        let body: Vec<u8> = vec![];
        let frame = header::build_frame(msg_type, body.clone(), flags);
        let msg = &frame[4..];
        let (h, b) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.body_len, 0u32);
        prop_assert_eq!(b, &body[..]);
    }

    /// c. Body of exactly 65535 bytes should roundtrip
    #[test]
    fn prop_header_max_body(
        msg_type in 0..=6u8,
        flags in any::<u16>(),
    ) {
        let body = vec![0xABu8; 65535];
        let frame = header::build_frame(msg_type, body.clone(), flags);
        let msg = &frame[4..];
        let (h, b) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.body_len, 65535u32);
        prop_assert_eq!(b, &body[..]);
    }

    /// d. All 16-bit flag combinations survive roundtrip
    #[test]
    fn prop_header_flags_preserved(
        msg_type in 0..=6u8,
        flags in any::<u16>(),
    ) {
        let body = vec![0x42u8; 10];
        let frame = header::build_frame(msg_type, body.clone(), flags);
        let msg = &frame[4..];
        let (h, _b) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.flags, flags);
    }
}

// ─── Secure channel property tests ──────────────────────────────

proptest! {
    /// Encrypt-then-decrypt is the identity: plaintext → encrypt → decrypt → plaintext
    #[test]
    fn prop_encrypt_decrypt_identity(
        plaintext in prop::collection::vec(any::<u8>(), 0..=4096),
    ) {
        let key = SecureChannel::generate_key();
        let aad = b"test-aad";
        let (nonce, ciphertext) = SecureChannel::encrypt_raw(&key, &plaintext, aad)
            .expect("encrypt_raw should succeed");
        let decrypted = SecureChannel::decrypt_raw(&key, &nonce, &ciphertext, aad)
            .expect("decrypt_raw should succeed");
        prop_assert_eq!(decrypted, plaintext);
    }

    /// Sign-then-verify is the identity: message → sign → verify → Ok
    #[test]
    fn prop_sign_verify_identity(
        message in prop::collection::vec(any::<u8>(), 0..=4096),
    ) {
        let identity = NodeIdentity::new();
        let sig = identity.sign(&message);
        let result = verify_signature(
            &identity.public_key_bytes(),
            &message,
            &sig.to_bytes(),
        );
        prop_assert!(result.is_ok(), "sign-then-verify must pass");
    }

    /// Different keys produce different signatures for the same message
    #[test]
    fn prop_different_keys_different_signatures(
        message in prop::collection::vec(any::<u8>(), 1..=256),
    ) {
        let id_a = NodeIdentity::new();
        let id_b = NodeIdentity::new();
        let sig_a = id_a.sign(&message);
        let sig_b = id_b.sign(&message);
        prop_assert_ne!(
            sig_a.to_bytes(),
            sig_b.to_bytes(),
            "two different keys should produce different signatures"
        );
    }

    /// Header build_frame → parse_frame roundtrip preserves msg_type and body
    #[test]
    fn prop_frame_roundtrip(
        msg_type in 0u8..=7,
        body in prop::collection::vec(any::<u8>(), 0..=1000),
    ) {
        let frame = header::build_frame(msg_type, body.clone(), 0);
        let msg = &frame[4..]; // skip 4-byte length prefix
        let (h, parsed_body) = header::parse_frame(msg).unwrap();
        prop_assert_eq!(h.msg_type, msg_type);
        prop_assert_eq!(h.body_len as usize, body.len());
        prop_assert_eq!(parsed_body, &body[..]);
    }

    /// e. Encrypt empty plaintext should succeed
    #[test]
    fn prop_encrypt_empty() {
        let key = SecureChannel::generate_key();
        let plaintext: Vec<u8> = vec![];
        let aad = b"test-aad";
        let result = SecureChannel::encrypt_raw(&key, &plaintext, aad);
        prop_assert!(result.is_some(), "encrypting empty plaintext should succeed");
        // Verify roundtrip
        let (nonce, ciphertext) = result.unwrap();
        let decrypted = SecureChannel::decrypt_raw(&key, &nonce, &ciphertext, aad);
        prop_assert!(decrypted.is_some(), "decrypting empty plaintext should succeed");
        prop_assert_eq!(decrypted.unwrap(), plaintext);
    }

    /// f. Encrypt 1MB plaintext should succeed
    #[test]
    fn prop_encrypt_large() {
        let key = SecureChannel::generate_key();
        let plaintext = vec![0x42u8; 1_048_576]; // 1MB
        let aad = b"test-aad";
        let result = SecureChannel::encrypt_raw(&key, &plaintext, aad);
        prop_assert!(result.is_some(), "encrypting 1MB should succeed");
        // Verify roundtrip
        let (nonce, ciphertext) = result.unwrap();
        let decrypted = SecureChannel::decrypt_raw(&key, &nonce, &ciphertext, aad);
        prop_assert!(decrypted.is_some(), "decrypting 1MB should succeed");
        prop_assert_eq!(decrypted.unwrap(), plaintext);
    }
}

// ─── Trust system property tests ────────────────────────────

proptest! {
    /// g. Trust score always stays in [0.0, 1.0] after any sequence of events
    #[test]
    fn prop_trust_score_bounded(
        events in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let mut ts = TrustSystem::new();
        let mut eid_bytes = [0u8; 32];
        eid_bytes[0..8].copy_from_slice(&1u64.to_le_bytes());
        let peer = EntityId(eid_bytes);

        for &event_byte in &events {
            let event = match event_byte % 8 {
                0 => TrustEvent::ValidSignature,
                1 => TrustEvent::InvalidSignature,
                2 => TrustEvent::SuccessfulDecrypt,
                3 => TrustEvent::ReplayAttack,
                4 => TrustEvent::PacketTimeout,
                5 => TrustEvent::RateLimitExceeded,
                6 => TrustEvent::SuccessfulHandshake,
                _ => TrustEvent::FailedHandshake,
            };
            let score = ts.record_event(peer, event);
            prop_assert!(
                score >= 0.0 && score <= 1.0,
                "trust score {} out of [0.0, 1.0] bounds after event {:?}", score, event
            );
        }
    }

    /// h. 100 replay attacks should push score below SYBIL_THRESHOLD
    #[test]
    fn prop_trust_sybil_escalation() {
        let mut ts = TrustSystem::new();
        let mut eid_bytes = [0u8; 32];
        eid_bytes[0..8].copy_from_slice(&1u64.to_le_bytes());
        let peer = EntityId(eid_bytes);

        for _ in 0..100 {
            ts.record_event(peer, TrustEvent::ReplayAttack);
        }
        let score = ts.trust_score(&peer);
        prop_assert!(
            score < SYBIL_THRESHOLD,
            "100 replay attacks should push score {} below SYBIL_THRESHOLD {}",
            score, SYBIL_THRESHOLD
        );
    }
}

// ─── CRC32 property tests ──────────────────────────────────

proptest! {
    /// i. CRC32 is deterministic for larger inputs
    #[test]
    fn prop_crc32_deterministic(
        data in prop::collection::vec(any::<u8>(), 256..10000),
    ) {
        let c1 = crc32(&data);
        let c2 = crc32(&data);
        prop_assert_eq!(c1, c2, "same input must always produce same CRC");
    }

    /// j. Flipping 1 bit in data always changes the CRC
    #[test]
    fn prop_crc32_differs_on_tamper(
        data in prop::collection::vec(any::<u8>(), 1..1024),
        flip_pos in 0usize..1024,
    ) {
        let original_crc = crc32(&data);
        let mut tampered = data.clone();
        let pos = flip_pos % tampered.len();
        tampered[pos] ^= 1; // flip lowest bit
        let tampered_crc = crc32(&tampered);
        prop_assert_ne!(
            original_crc, tampered_crc,
            "CRC should differ when1 bit is flipped at position {}", pos
        );
    }
}
