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
