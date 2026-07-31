//! Integration tests for neuron-wire protocol roundtrips.
//! These compile as a separate crate, testing the public library API.

use neuron_wire::header::{self, HeaderError, MessageHeader};

#[test]
fn integration_ping_roundtrip() {
    let ping = header::build_ping();
    assert!(ping.len() > 4 + 16);
    let (h, b) = header::parse_frame(&ping[4..]).unwrap();
    assert_eq!(h.msg_type, 0); // Ping
    assert_eq!(h.body_len, 0);
    assert!(b.is_empty());
}

#[test]
fn integration_pong_roundtrip() {
    let pong = header::build_pong();
    let (h, b) = header::parse_frame(&pong[4..]).unwrap();
    assert_eq!(h.msg_type, 1); // Pong
    assert!(b.is_empty());
}

#[test]
fn integration_custom_frame() {
    let body = b"hello neuron-wire".to_vec();
    let frame = header::build_frame(5, body.clone(), 0x0004);
    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.msg_type, 5);
    assert_eq!(h.flags, 0x0004);
    assert_eq!(b, &body[..]);
}

#[test]
fn integration_frame_length_prefix() {
    let body = vec![0u8; 256];
    let frame = header::build_frame(3, body, 0);
    let len_field = u32::from_le_bytes(frame[0..4].try_into().unwrap());
    assert_eq!(len_field as usize, frame.len());
}

#[test]
fn integration_header_from_bytes_across_frames() {
    // Build 10 frames with random-ish bodies, verify each header is valid
    for i in 0..10u8 {
        let body = vec![i; (i as usize + 1) * 16];
        let frame = header::build_frame(i % 7, body, 0);
        let h = MessageHeader::from_bytes(&frame[4..]).unwrap();
        assert_eq!(h.msg_type, i % 7);
        assert!(h.validate().is_ok());
    }
}

#[test]
fn integration_invalid_frame_rejected() {
    // Too-short buffer
    let err = header::parse_frame(&[0u8; 4]).unwrap_err();
    match err {
        HeaderError::ShortBuffer(n) => assert_eq!(n, 4),
        _ => panic!("expected ShortBuffer"),
    }
}

#[test]
fn integration_bad_magic_rejected() {
    let h = MessageHeader::new(0, 0, 0);
    let mut bytes = h.to_bytes();
    bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // corrupt magic
    let err = MessageHeader::from_bytes(&bytes).unwrap_err();
    match err {
        HeaderError::BadMagic(_) => {}
        _ => panic!("expected BadMagic"),
    }
}
// ═══════════════════════════════════════════════════════════════
// Additional Integration Tests — Expanded Test Suite
// ═══════════════════════════════════════════════════════════════

use neuron_wire::audit::{AuditEventType, AuditLog};
use neuron_wire::components::EntityId;
use neuron_wire::identity::NodeIdentity;
use neuron_wire::secure_channel::SecureChannel;
use neuron_wire::transport::TransportHeader;
use neuron_wire::trust::{TrustEvent, TrustSystem, INITIAL_TRUST};

/// Simple xorshift32 PRNG — no external crate dependency needed.
fn xorshift32(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

// ── Test (a): Trust Event Roundtrip ────────────────────────────

/// Record trust events in sequence and verify that the resulting score
/// changes match the documented deltas (clamped to [0.0, 1.0]).
#[test]
fn integration_trust_event_roundtrip() {
    let mut ts = TrustSystem::new();
    let peer = EntityId([1u8; 32]);

    // Initial score is INITIAL_TRUST (0.5) for an unknown peer
    let score0 = ts.trust_score(&peer);
    assert!(
        (score0 - INITIAL_TRUST).abs() < 0.01,
        "initial score should be ~0.5, got {}",
        score0
    );

    // ValidSignature: +0.05
    let s1 = ts.record_event(peer, TrustEvent::ValidSignature);
    assert!(
        (s1 - 0.55).abs() < 0.02,
        "after ValidSignature expected ~0.55, got {}",
        s1
    );

    // SuccessfulDecrypt: +0.02
    let s2 = ts.record_event(peer, TrustEvent::SuccessfulDecrypt);
    assert!(
        (s2 - 0.57).abs() < 0.02,
        "after SuccessfulDecrypt expected ~0.57, got {}",
        s2
    );

    // InvalidSignature: -0.50
    let s3 = ts.record_event(peer, TrustEvent::InvalidSignature);
    assert!(
        (s3 - 0.07).abs() < 0.02,
        "after InvalidSignature expected ~0.07, got {}",
        s3
    );

    // ReplayAttack: -0.80 (should clamp to 0.0)
    let s4 = ts.record_event(peer, TrustEvent::ReplayAttack);
    assert!(s4 <= 0.02, "after ReplayAttack expected ~0.0, got {}", s4);

    // SuccessfulHandshake: +0.10
    let s5 = ts.record_event(peer, TrustEvent::SuccessfulHandshake);
    assert!(
        (s5 - 0.10).abs() < 0.02,
        "after SuccessfulHandshake expected ~0.10, got {}",
        s5
    );

    // PacketTimeout: -0.10
    let s6 = ts.record_event(peer, TrustEvent::PacketTimeout);
    assert!(s6 <= 0.02, "after PacketTimeout expected ~0.0, got {}", s6);

    // FailedHandshake: -0.20 (clamp at 0.0)
    let s7 = ts.record_event(peer, TrustEvent::FailedHandshake);
    assert!(
        s7 <= 0.02,
        "after FailedHandshake expected ~0.0, got {}",
        s7
    );

    // RateLimitExceeded: -0.05 (already at 0.0, stays at 0.0)
    let s8 = ts.record_event(peer, TrustEvent::RateLimitExceeded);
    assert!(
        s8 <= 0.02,
        "after RateLimitExceeded expected ~0.0, got {}",
        s8
    );

    // Verify stats
    let stats = ts.stats();
    assert_eq!(stats.total_peers, 1);
    assert_eq!(stats.sybil_peers, 1); // score < 0.2
}

// ── Test (b): Trust Rate Limit Enforcement ─────────────────────

/// Burst traffic from a new peer (initial trust = 0.5) and verify that
/// per-peer rate limiting activates within the expected packet window.
#[test]
fn integration_trust_rate_limit_enforcement() {
    let mut ts = TrustSystem::new();
    let peer = EntityId([42u8; 32]);

    // New peer at INITIAL_TRUST (0.5) is between SYBIL_THRESHOLD (0.2)
    // and TRUSTED_THRESHOLD (0.7), so limit = RATE_LIMIT_BURST = 10.
    let mut rate_limited_at = None;
    for i in 0..50u32 {
        if ts.check_rate_limit(&peer) {
            rate_limited_at = Some(i);
            break;
        }
    }

    let limited_at = rate_limited_at.expect("peer should be rate-limited within 50 packets");
    // Should trigger around packet 11 (10 allowed + 1st rejected)
    assert!(
        limited_at <= 15,
        "rate limiting should kick in by packet 15, got {}",
        limited_at
    );

    // Verify trust score decreased (RateLimitExceeded event was recorded)
    let score = ts.trust_score(&peer);
    assert!(
        score < INITIAL_TRUST,
        "rate limiting should reduce trust: expected < 0.5, got {}",
        score
    );
}

// ── Test (c): Trust Save / Load ────────────────────────────────

/// Save trust scores to a binary file, load them into a brand-new
/// TrustSystem, and verify that all scores are preserved.
#[test]
fn integration_trust_save_load() {
    let mut ts = TrustSystem::new();
    let peer1 = EntityId([1u8; 32]);
    let peer2 = EntityId([2u8; 32]);
    let peer3 = EntityId([3u8; 32]);

    // Build up distinct trust scores for each peer
    ts.record_event(peer1, TrustEvent::ValidSignature);
    ts.record_event(peer1, TrustEvent::ValidSignature);
    ts.record_event(peer1, TrustEvent::SuccessfulHandshake);
    // peer1: ~0.5 + 0.05 + 0.05 + 0.10 = ~0.70

    ts.record_event(peer2, TrustEvent::InvalidSignature);
    // peer2: ~0.5 - 0.50 = ~0.0

    ts.record_event(peer3, TrustEvent::SuccessfulDecrypt);
    ts.record_event(peer3, TrustEvent::SuccessfulDecrypt);
    ts.record_event(peer3, TrustEvent::SuccessfulDecrypt);
    // peer3: ~0.5 + 0.02 + 0.02 + 0.02 = ~0.56

    let s1_before = ts.trust_score(&peer1);
    let s2_before = ts.trust_score(&peer2);
    let s3_before = ts.trust_score(&peer3);

    // Save to a temp file
    let path = std::env::temp_dir().join("neuron_wire_trust_integration_test.dat");
    let saved = ts.save_to_file(path.to_str().unwrap()).unwrap();
    assert_eq!(saved, 3, "should save 3 peers");

    // Load into a brand-new TrustSystem
    let mut ts2 = TrustSystem::new();
    let loaded = ts2.load_from_file(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded, 3, "should load 3 peers");

    // Verify scores preserved (tolerance accounts for tiny time decay)
    let s1_after = ts2.trust_score(&peer1);
    let s2_after = ts2.trust_score(&peer2);
    let s3_after = ts2.trust_score(&peer3);

    assert!(
        (s1_after - s1_before).abs() < 0.02,
        "peer1 score mismatch: before={} after={}",
        s1_before,
        s1_after
    );
    assert!(
        (s2_after - s2_before).abs() < 0.02,
        "peer2 score mismatch: before={} after={}",
        s2_before,
        s2_after
    );
    assert!(
        (s3_after - s3_before).abs() < 0.02,
        "peer3 score mismatch: before={} after={}",
        s3_before,
        s3_after
    );

    // Verify trust classification is preserved
    assert!(ts2.is_trusted(&peer1), "peer1 should be trusted");
    assert!(!ts2.is_trusted(&peer2), "peer2 should be untrusted (sybil)");

    // Clean up
    let _ = std::fs::remove_file(&path);
}

// ── Test (d): Header Stress Test ───────────────────────────────

/// Build 10,000 frames with pseudo-random message types and body sizes,
/// then parse every single one back. Verifies 100% round-trip success.
#[test]
fn integration_header_stress() {
    let mut rng_state: u32 = 0xDEAD_BEEF;

    for _ in 0..10_000 {
        let msg_type = (xorshift32(&mut rng_state) % 7) as u8;
        let body_len = (xorshift32(&mut rng_state) % 4096) as usize;
        let flags = (xorshift32(&mut rng_state) & 0xFFFF) as u16;

        let body: Vec<u8> = (0..body_len)
            .map(|_| xorshift32(&mut rng_state) as u8)
            .collect();

        let frame = header::build_frame(msg_type, body.clone(), flags);
        let (h, b) = header::parse_frame(&frame[4..])
            .expect("parse_frame should never fail for valid frames");

        assert_eq!(h.msg_type, msg_type, "msg_type mismatch");
        assert_eq!(h.flags, flags, "flags mismatch");
        assert_eq!(b.len(), body_len, "body length mismatch");
        if body_len > 0 {
            assert_eq!(b, &body[..], "body content mismatch");
        }
    }
}

// ── Test (e): Transport Header Zero-Copy Roundtrip ─────────────

/// Verify TransportHeader to_bytes/from_bytes roundtrip for boundary
/// and typical values — the zero-copy reinterpretation must be lossless.
#[test]
fn integration_transport_header_zero_copy() {
    let test_cases: [(u32, u32, u32, u32); 5] = [
        (0, 0, 0, 0),
        (1, 0, 0, 1000),
        (u32::MAX, u32::MAX, 0xFFFFFFFF, u32::MAX),
        (42, 100, 0x000000FF, 999_999),
        (1_000_000, 500_000, 0x0000_0001, 123_456),
    ];

    for (seq, ack, bitfield, ts) in &test_cases {
        let header = TransportHeader::new(*seq, *ack, *bitfield, *ts);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), TransportHeader::SIZE);

        // SAFETY: to_bytes() always produces a 16-byte buffer, which is >= TransportHeader::SIZE
        let parsed = unsafe { TransportHeader::from_bytes(&bytes) };
        assert_eq!(parsed.sequence_number, *seq, "seq mismatch");
        assert_eq!(parsed.ack_number, *ack, "ack mismatch");
        assert_eq!(parsed.ack_bitfield, *bitfield, "bitfield mismatch");
        assert_eq!(parsed.timestamp, *ts, "timestamp mismatch");
    }
}

// ── Test (f): Channel Multi-Peer ───────────────────────────────

/// Create a single SecureChannel with 10 peers (separate sessions),
/// then encrypt a message for each peer session and verify success.
#[test]
fn integration_channel_multi_peer() {
    let mut channel = SecureChannel::new();
    let local_identity = NodeIdentity::new();
    let mut session_ids: Vec<[u8; 32]> = Vec::new();

    // Handshake with 10 distinct peers
    for i in 0..10u8 {
        let mut seed = [0u8; 32];
        seed[0] = i;
        seed[1] = 0xFF;
        seed[2] = i.wrapping_mul(7);
        let peer_id = NodeIdentity::from_seed(&seed);
        let peer_pk = peer_id.public_key_bytes();

        let session_id = channel.handshake(&local_identity, peer_pk);
        session_ids.push(session_id);

        // Verify session was created
        assert!(
            channel.has_session(&session_id),
            "session for peer {} should exist",
            i
        );
    }

    // Verify we have exactly 10 sessions
    assert_eq!(channel.session_count(), 10);

    // Encrypt a unique message for each peer session
    for (i, session_id) in session_ids.iter().enumerate() {
        let plaintext = format!("secret message to peer {}", i);
        let result = channel.encrypt(session_id, plaintext.as_bytes(), b"");
        assert!(
            result.is_some(),
            "encryption should succeed for session {}",
            i
        );
        let (nonce, ciphertext) = result.unwrap();
        assert_eq!(nonce.len(), 16, "nonce should be 16 bytes");
        assert!(
            !ciphertext.is_empty(),
            "ciphertext should be non-empty for session {}",
            i
        );
        // Ciphertext should be different from plaintext
        assert_ne!(ciphertext, plaintext.as_bytes());
    }

    // Verify all sessions have the expected peer public keys
    for (i, session_id) in session_ids.iter().enumerate() {
        assert!(
            channel.peer_public_key(session_id).is_some(),
            "peer public key should be retrievable for session {}",
            i
        );
    }
}

// ── Test (g): Audit Chain Growth ───────────────────────────────

/// Append 1,000 entries to the audit log and verify hash chain integrity
/// at regular intervals, confirming the chain never breaks.
#[test]
fn integration_audit_chain_growth() {
    let mut log = AuditLog::new();

    for i in 0..1000u64 {
        let peer = if i % 3 == 0 {
            Some(EntityId([(i % 256) as u8; 32]))
        } else {
            None
        };

        let seq = log.append(
            AuditEventType::PeerDiscovered,
            &format!("peer discovery #{}", i),
            peer,
        );
        assert_eq!(seq, i, "sequence number should be monotonically increasing");

        // Verify integrity every 100 entries
        if i % 100 == 0 {
            assert!(
                log.verify_integrity(),
                "hash chain integrity failed at entry {}",
                i
            );
        }
    }

    // Final integrity check
    assert!(
        log.verify_integrity(),
        "final hash chain integrity check failed"
    );
    assert_eq!(log.total_entries(), 1000);

    // Verify genesis hash is preserved
    assert!(
        log.genesis_hash().is_some(),
        "genesis hash should be set after 1000 entries"
    );

    // Verify current hash is non-zero
    let current = log.current_hash();
    assert_ne!(current, [0u8; 32], "current hash should be non-zero");
}

// ── Test (h): EntityId Determinism ─────────────────────────────

/// Verify that the same Ed25519 seed always produces the same EntityId,
/// confirming deterministic derivation across multiple instantiations.
#[test]
fn integration_entity_id_determinism() {
    let seed = [42u8; 32];

    // Create the same identity 5 times from the same seed
    let id1 = NodeIdentity::from_seed(&seed);
    let id2 = NodeIdentity::from_seed(&seed);
    let id3 = NodeIdentity::from_seed(&seed);
    let id4 = NodeIdentity::from_seed(&seed);
    let id5 = NodeIdentity::from_seed(&seed);

    // All EntityIds must be identical
    assert_eq!(id1.entity_id(), id2.entity_id());
    assert_eq!(id2.entity_id(), id3.entity_id());
    assert_eq!(id3.entity_id(), id4.entity_id());
    assert_eq!(id4.entity_id(), id5.entity_id());

    // All public keys must be identical
    assert_eq!(id1.public_key_bytes(), id2.public_key_bytes());
    assert_eq!(id2.public_key_bytes(), id3.public_key_bytes());
    assert_eq!(id3.public_key_bytes(), id4.public_key_bytes());
    assert_eq!(id4.public_key_bytes(), id5.public_key_bytes());

    // Derive EntityId from public key independently — must match
    let derived = neuron_wire::identity::entity_id_from_public_key(&id1.public_key_bytes());
    assert_eq!(
        id1.entity_id(),
        derived,
        "EntityId from from_seed() must match entity_id_from_public_key()"
    );

    // Different seeds must produce different EntityIds
    let other_seed = [99u8; 32];
    let other = NodeIdentity::from_seed(&other_seed);
    assert_ne!(
        id1.entity_id(),
        other.entity_id(),
        "different seeds must produce different EntityIds"
    );
}

// ── Test (i): Frame Length Validation ──────────────────────────

/// Verify that frames with truncated or inconsistent lengths are
/// handled gracefully — valid frames parse correctly, corrupted ones
/// return descriptive errors, and oversized bodies are rejected.
#[test]
fn integration_frame_length_validation() {
    // Case 1: Valid frame with body parses correctly
    let body = vec![0xAB; 256];
    let frame = header::build_frame(5, body.clone(), 0);
    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.body_len, 256);
    assert_eq!(b.len(), 256);
    assert_eq!(b, &body[..]);

    // Case 2: Length prefix matches total frame size
    let len_field = u32::from_le_bytes(frame[0..4].try_into().unwrap());
    assert_eq!(len_field as usize, frame.len());

    // Case 3: Truncated buffer (less than 16 bytes) returns ShortBuffer
    for cut in [0, 1, 4, 8, 15] {
        let short = &frame[4..4 + cut];
        match header::parse_frame(short) {
            Err(HeaderError::ShortBuffer(n)) => assert_eq!(n, cut),
            other => panic!(
                "expected ShortBuffer({}) for {}-byte buffer, got {:?}",
                cut, cut, other
            ),
        }
    }

    // Case 4: Valid header with zero body
    let ping = header::build_ping();
    let (h, b) = header::parse_frame(&ping[4..]).unwrap();
    assert_eq!(h.body_len, 0);
    assert!(b.is_empty());

    // Case 5: Large body (10,000 bytes) parses correctly
    let big_body = vec![0xFF; 10_000];
    let big_frame = header::build_frame(5, big_body, 0);
    let (h, b) = header::parse_frame(&big_frame[4..]).unwrap();
    assert_eq!(h.body_len, 10_000);
    assert_eq!(b.len(), 10_000);

    // Case 6: Header with body_len exceeding MAX_BODY_SIZE is rejected
    // even if the raw bytes are present
    let oversized_header =
        neuron_wire::header::MessageHeader::new(5, neuron_wire::MAX_BODY_SIZE + 1, 0);
    let err =
        neuron_wire::header::MessageHeader::from_bytes(&oversized_header.to_bytes()).unwrap_err();
    match err {
        HeaderError::BodyTooLarge(s) => assert!(s > neuron_wire::MAX_BODY_SIZE),
        other => panic!("expected BodyTooLarge, got {:?}", other),
    }
}

// ── Test (j): Disconnect Reason Roundtrip ──────────────────────

/// Verify that every disconnect reason code survives a full
/// build_frame → parse_frame roundtrip with its associated message.
#[test]
fn integration_disconnect_reasons() {
    let reasons: &[(u8, &str)] = &[
        (header::disconnect_reason::SHUTDOWN, "node shutting down"),
        (header::disconnect_reason::RESTART, "restarting for update"),
        (
            header::disconnect_reason::TOO_MANY_PEERS,
            "connection limit reached",
        ),
        (
            header::disconnect_reason::VERSION_MISMATCH,
            "protocol v3 not supported",
        ),
        (
            header::disconnect_reason::RATE_LIMITED,
            "burst traffic exceeded",
        ),
        (header::disconnect_reason::ADMIN, "admin override"),
    ];

    for &(code, msg) in reasons {
        // Build body: [reason_code, msg_len, msg_bytes...]
        let mut body = Vec::with_capacity(2 + msg.len());
        body.push(code);
        body.push(msg.len() as u8);
        body.extend_from_slice(msg.as_bytes());

        let frame = header::build_frame(header::msg_type::DISCONNECT, body, 0);
        let (h, b) =
            header::parse_frame(&frame[4..]).expect("disconnect frame should parse successfully");

        assert_eq!(
            h.msg_type,
            header::msg_type::DISCONNECT,
            "msg_type should be DISCONNECT"
        );
        assert_eq!(b[0], code, "reason code mismatch for code {}", code);

        let parsed_msg_len = b[1] as usize;
        assert_eq!(parsed_msg_len, msg.len(), "message length mismatch");
        assert_eq!(
            &b[2..2 + parsed_msg_len],
            msg.as_bytes(),
            "message content mismatch for code {}",
            code
        );
    }

    // Verify all 6 reason codes are distinct
    let codes: Vec<u8> = reasons.iter().map(|(c, _)| *c).collect();
    let mut sorted_codes = codes.clone();
    sorted_codes.sort();
    sorted_codes.dedup();
    assert_eq!(
        sorted_codes.len(),
        codes.len(),
        "all disconnect codes must be unique"
    );
}
