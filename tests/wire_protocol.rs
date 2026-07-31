//! Wire protocol roundtrip tests.
//!
//! Tests the full pipeline: build → serialize → send → recv → parse → verify.
//! These tests exercise the protocol at the packet level without needing
//! a running engine.

use neuron_wire::header::{self, MessageHeader, HEADER_SIZE};
use neuron_wire::identity::NodeIdentity;

#[test]
fn wire_ping_pong_roundtrip() {
    let ping = header::build_ping();
    assert!(ping.len() >= 4 + HEADER_SIZE);

    // Verify frame length field
    let frame_len = u32::from_le_bytes(ping[0..4].try_into().unwrap());
    assert_eq!(frame_len as usize, ping.len());

    // Parse
    let (h, body) = header::parse_frame(&ping[4..]).unwrap();
    assert_eq!(h.msg_type, 0); // PING
    assert!(body.is_empty());

    // Pong
    let pong = header::build_pong();
    let (h, body) = header::parse_frame(&pong[4..]).unwrap();
    assert_eq!(h.msg_type, 1); // PONG
    assert!(body.is_empty());
}

#[test]
fn wire_all_message_types() {
    // Test every defined message type
    let msg_types = [
        (0, "PING"),
        (1, "PONG"),
        (7, "DHT_PING"),
        (8, "DHT_PONG"),
        (9, "FIND_NODE"),
        (10, "NODES"),
        (20, "GRADIENT"),
        (21, "GRADIENT_ACK"),
        (30, "HEARTBEAT"),
        (40, "DISCONNECT"),
    ];

    for (mt, name) in &msg_types {
        let body = format!("test-body-{}", name).into_bytes();
        let frame = header::build_frame(*mt, body.clone(), 0);
        let (h, b) = header::parse_frame(&frame[4..]).unwrap();
        assert_eq!(h.msg_type, *mt, "{} msg_type mismatch", name);
        assert_eq!(b, &body[..], "{} body mismatch", name);
    }
}

#[test]
fn wire_empty_body() {
    let frame = header::build_frame(5, Vec::new(), 0);
    let (h, body) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.body_len, 0);
    assert!(body.is_empty());
}

#[test]
fn wire_large_body() {
    // Max body size that fits in u32
    let body = vec![0xABu8; 65535];
    let frame = header::build_frame(5, body.clone(), 0);
    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.body_len, 65535);
    assert_eq!(b, &body[..]);
}

#[test]
fn wire_flags_preserved() {
    // Test all 16-bit flag combinations (sample)
    let flags_to_test = [0, 1, 2, 3, 4, 0x0002, 0x0001, 0xFFFF, 0x8000, 0x00FF];
    let body = b"flag-test".to_vec();

    for &flags in &flags_to_test {
        let frame = header::build_frame(5, body.clone(), flags);
        let (h, _) = header::parse_frame(&frame[4..]).unwrap();
        assert_eq!(
            h.flags, flags,
            "flags 0x{:04X} not preserved",
            flags
        );
    }
}

#[test]
fn wire_authenticated_flag() {
    let body = b"auth-test".to_vec();
    let frame = header::build_frame(5, body.clone(), header::FLAG_AUTHENTICATED);
    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
    assert_ne!(h.flags & header::FLAG_AUTHENTICATED, 0);
    assert_eq!(b, &body[..]);
}

#[test]
fn wire_signed_packet_roundtrip() {
    // Full pipeline: build → sign → serialize → parse → verify
    let alice = NodeIdentity::new();
    let body = b"gradient-update-v1".to_vec();

    // Build frame
    let frame = header::build_frame(20, body.clone(), 0);
    let (h, payload) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.msg_type, 20); // GRADIENT

    // Sign
    let seq = 1u64;
    let ts = 1_700_000_000u64;
    let body_hash = neuron_wire::crc::crc32(payload);
    let sig = alice.sign_packet(seq, ts, &body_hash.to_le_bytes());
    let sig_bytes: [u8; 64] = sig.to_bytes();

    // Verify
    let verify_result = neuron_wire::identity::verify_packet_signature(
        &alice.public_key_bytes(),
        seq,
        ts,
        &body_hash.to_le_bytes(),
        &sig_bytes,
    );
    assert!(verify_result.is_ok(), "signature must verify");

    // Wrong key fails
    let bob = NodeIdentity::new();
    let wrong_verify = neuron_wire::identity::verify_packet_signature(
        &bob.public_key_bytes(),
        seq,
        ts,
        &body_hash.to_le_bytes(),
        &sig_bytes,
    );
    assert!(wrong_verify.is_err(), "wrong key must fail");
}

#[test]
fn wire_authenticated_frame_with_signature() {
    // Build an authenticated frame: 32B pubkey + 64B sig + body
    let alice = NodeIdentity::new();
    let body = b"authenticated-gradient".to_vec();

    // Sign the body
    let sig = alice.sign(&body);
    let sig_bytes = sig.to_bytes();

    // Build auth prefix
    let mut authed_body = Vec::with_capacity(96 + body.len());
    authed_body.extend_from_slice(&alice.public_key_bytes());
    authed_body.extend_from_slice(&sig_bytes);
    authed_body.extend_from_slice(body);

    let frame = header::build_frame(20, authed_body, header::FLAG_AUTHENTICATED);
    let (h, payload) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.msg_type, 20);
    assert_ne!(h.flags & header::FLAG_AUTHENTICATED, 0);

    // Extract and verify
    let pk: [u8; 32] = payload[..32].try_into().unwrap();
    let sig: [u8; 64] = payload[32..96].try_into().unwrap();
    let inner_body = &payload[96..];

    assert!(neuron_wire::identity::verify_signature(&pk, inner_body, &sig).is_ok());
    assert_eq!(inner_body, b"authenticated-gradient");
}

#[test]
fn wire_encrypted_frame_roundtrip() {
    use neuron_wire::secure_channel::SecureChannel;

    let alice = NodeIdentity::new();
    let bob = NodeIdentity::new();

    // Establish channel
    let mut alice_chan = SecureChannel::new();
    let mut bob_chan = SecureChannel::new();
    let sid_a = alice_chan.handshake(&alice, bob.public_key_bytes());
    let sid_b = bob_chan.handshake(&bob, alice.public_key_bytes());

    let peer_addr: std::net::SocketAddr = "127.0.0.1:9001".parse().unwrap();
    alice_chan.register_peer_session(peer_addr, sid_a);

    // Build body
    let body = b"encrypted-gradient-data".to_vec();
    let frame = header::build_frame(20, body, 0);
    let (h, payload) = header::parse_frame(&frame[4..]).unwrap();

    // Sign
    let sig = alice.sign(payload);
    let mut authed = Vec::with_capacity(96 + payload.len());
    authed.extend_from_slice(&alice.public_key_bytes());
    authed.extend_from_slice(&sig.to_bytes());
    authed.extend_from_slice(payload);

    // Encrypt
    let session_id = alice_chan.get_session_by_peer(&peer_addr).unwrap();
    let (nonce, ct) = alice_chan.encrypt(&session_id, &authed, &[]).unwrap();

    // Build encrypted frame
    let mut enc_body = Vec::with_capacity(16 + ct.len());
    enc_body.extend_from_slice(&nonce);
    enc_body.extend_from_slice(&ct);

    let enc_frame = header::build_frame(
        h.msg_type,
        enc_body,
        header::FLAG_AUTHENTICATED | header::FLAG_ENCRYPTED,
    );

    // Bob receives and decrypts
    let (recv_h, recv_payload) = header::parse_frame(&enc_frame[4..]).unwrap();
    assert_ne!(recv_h.flags & header::FLAG_ENCRYPTED, 0);

    let recv_nonce: [u8; 16] = recv_payload[..16].try_into().unwrap();
    let recv_ct = &recv_payload[16..];
    let decrypted = bob_chan.decrypt(&sid_b, &recv_nonce, recv_ct, &[]).unwrap();

    // Verify signature
    let recv_pk: [u8; 32] = decrypted[..32].try_into().unwrap();
    let recv_sig: [u8; 64] = decrypted[32..96].try_into().unwrap();
    let recv_body = &decrypted[96..];

    assert!(neuron_wire::identity::verify_signature(&recv_pk, recv_body, &recv_sig).is_ok());
    assert_eq!(recv_pk, alice.public_key_bytes());
    assert_eq!(recv_body, b"encrypted-gradient-data");
}

#[test]
fn wire_disconnect_frame() {
    let reason = neuron_wire::header::disconnect_reason::TOO_MANY_PEERS;
    let body = vec![reason];
    let frame = header::build_frame(
        neuron_wire::header::msg_type::DISCONNECT,
        body,
        0,
    );
    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.msg_type, neuron_wire::header::msg_type::DISCONNECT);
    assert_eq!(b[0], neuron_wire::header::disconnect_reason::TOO_MANY_PEERS);
}

#[test]
fn wire_heartbeat_empty_payload() {
    let frame = header::build_frame(
        neuron_wire::header::msg_type::HEARTBEAT,
        Vec::new(),
        0,
    );
    let (h, body) = header::parse_frame(&frame[4..]).unwrap();
    assert_eq!(h.msg_type, neuron_wire::header::msg_type::HEARTBEAT);
    assert!(body.is_empty());
}

#[test]
fn wire_crc32_integrity() {
    // Verify CRC32 detects bit flips
    let body = b"integrity-test-data".to_vec();
    let crc1 = neuron_wire::crc::crc32(&body);

    let mut tampered = body.clone();
    tampered[0] ^= 0xFF;
    let crc2 = neuron_wire::crc::crc32(&tampered);

    assert_ne!(crc1, crc2, "CRC must differ on tampered data");
}

#[test]
fn wire_concurrent_frame_build_parse() {
    use std::thread;

    // Build and parse frames concurrently from multiple threads
    let handles: Vec<_> = (0..8)
        .map(|id| {
            thread::spawn(move || {
                for i in 0..1000 {
                    let body = format!("thread-{}-msg-{}", id, i).into_bytes();
                    let frame = header::build_frame((id % 40) as u8, body.clone(), 0);
                    let (h, b) = header::parse_frame(&frame[4..]).unwrap();
                    assert_eq!(h.msg_type, (id % 40) as u8);
                    assert_eq!(b, &body[..]);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
