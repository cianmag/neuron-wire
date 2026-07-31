//! Criterion benchmarks for neuron-wire protocol hot paths with statistical rigor.
//!
//! Benchmarks cover the critical protocol stack:
//!
//! - **header_roundtrip** — build_frame + parse_frame for various body sizes
//! - **transport_header** — TransportHeader from_bytes + to_bytes roundtrip
//! - **crypto_bench** — SecureChannel encrypt + decrypt for various payload sizes
//! - **trust_bench** — TrustSystem record_event for all event types
//! - **crc32_bench** — CRC-32 checksum for various data sizes
//! - **throughput_bench** — full message pipeline: build + sign + encrypt + parse + verify + decrypt

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use neuron_wire::components::EntityId;

// ═══════════════════════════════════════════════════════════════
// (a) header_roundtrip — build_frame + parse_frame
// ═══════════════════════════════════════════════════════════════

fn header_roundtrip(c: &mut Criterion) {
    let sizes: &[usize] = &[64, 256, 1024, 4096, 16384];

    let mut group = c.benchmark_group("header_roundtrip");
    for &size in sizes {
        let body = vec![0x42u8; size];
        group.throughput(Throughput::Bytes(size as u64 + 16)); // header + body
        group.bench_function(format!("build_and_parse_{}B", size), |b| {
            b.iter(|| {
                let frame = black_box(neuron_wire::header::build_frame(
                    black_box(20), // GRADIENT message type
                    black_box(body.clone()),
                    black_box(0),
                ));
                // Skip 4-byte length prefix, parse header + body
                let msg = &frame[4..];
                let (header, parsed_body) =
                    black_box(neuron_wire::header::parse_frame(black_box(msg))).unwrap();
                black_box((header.msg_type, parsed_body.len()));
            })
        });
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════
// (b) transport_header — from_bytes + to_bytes roundtrip
// ═══════════════════════════════════════════════════════════════

fn transport_header(c: &mut Criterion) {
    use neuron_wire::transport::TransportHeader;

    let mut group = c.benchmark_group("transport_header");

    // to_bytes: serialize a transport header
    group.bench_function("to_bytes", |b| {
        let header = TransportHeader::new(1000, 999, 0xFF, 42);
        b.iter(|| black_box(black_box(&header).to_bytes()))
    });

    // from_bytes: deserialize a transport header (unsafe zero-copy)
    group.bench_function("from_bytes", |b| {
        let header = TransportHeader::new(1000, 999, 0xFF, 42);
        let bytes = header.to_bytes();
        b.iter(|| {
            let parsed = unsafe { TransportHeader::from_bytes(black_box(&bytes)) };
            black_box(parsed.sequence_number);
        })
    });

    // Roundtrip: to_bytes + from_bytes
    group.bench_function("roundtrip", |b| {
        let header = TransportHeader::new(1000, 999, 0xFF, 42);
        b.iter(|| {
            let bytes = black_box(&header).to_bytes();
            let parsed = unsafe { TransportHeader::from_bytes(black_box(&bytes)) };
            black_box((parsed.sequence_number, parsed.ack_number));
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
// (c) crypto_bench — SecureChannel encrypt + decrypt
// ═══════════════════════════════════════════════════════════════

fn crypto_bench(c: &mut Criterion) {
    use neuron_wire::identity::NodeIdentity;
    use neuron_wire::secure_channel::SecureChannel;

    let sizes: &[usize] = &[64, 256, 1024, 4096];
    let alice_id = NodeIdentity::new();
    let bob_id = NodeIdentity::new();
    let mut alice_channel = SecureChannel::new();
    let mut bob_channel = SecureChannel::new();
    let alice_pub = alice_id.public_key_bytes();
    let bob_pub = bob_id.public_key_bytes();
    let session_a = alice_channel.handshake(&alice_id, bob_pub);
    let session_b = bob_channel.handshake(&bob_id, alice_pub);

    // encrypt + decrypt as a pair per size
    for &size in sizes {
        let mut group = c.benchmark_group(format!("crypto_{}B", size));
        group.throughput(Throughput::Bytes(size as u64));

        let plaintext = vec![0xABu8; size];
        let associated_data = b"nwp";

        // Pre-encrypt to get a nonce + ciphertext for decrypt bench
        let (nonce, ciphertext) = alice_channel
            .encrypt(&session_a, &plaintext, associated_data)
            .expect("encrypt must succeed");

        group.bench_function("encrypt", |b| {
            b.iter_batched(
                || plaintext.clone(),
                |pt| {
                    black_box(
                        alice_channel
                            .encrypt(
                                black_box(&session_a),
                                black_box(&pt),
                                black_box(associated_data),
                            )
                            .unwrap(),
                    )
                },
                BatchSize::SmallInput,
            )
        });

        group.bench_function("decrypt", |b| {
            b.iter_batched(
                || (nonce, ciphertext.clone()),
                |(n, ct)| {
                    black_box(
                        bob_channel
                            .decrypt(
                                black_box(&session_b),
                                black_box(&n),
                                black_box(&ct),
                                black_box(associated_data),
                            )
                            .unwrap(),
                    )
                },
                BatchSize::SmallInput,
            )
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════
// (d) trust_bench — TrustSystem record_event for all event types
// ═══════════════════════════════════════════════════════════════

fn trust_bench(c: &mut Criterion) {
    use neuron_wire::trust::{TrustEvent, TrustSystem};

    let events: &[(TrustEvent, &str)] = &[
        (TrustEvent::ValidSignature, "valid_signature"),
        (TrustEvent::InvalidSignature, "invalid_signature"),
        (TrustEvent::SuccessfulDecrypt, "successful_decrypt"),
        (TrustEvent::ReplayAttack, "replay_attack"),
        (TrustEvent::PacketTimeout, "packet_timeout"),
        (TrustEvent::RateLimitExceeded, "rate_limit_exceeded"),
        (TrustEvent::SuccessfulHandshake, "successful_handshake"),
        (TrustEvent::FailedHandshake, "failed_handshake"),
    ];

    let mut group = c.benchmark_group("trust_record_event");

    for &(event, name) in events {
        let peer = EntityId([1u8; 32]);
        group.bench_function(name, |b| {
            b.iter_batched(
                || TrustSystem::new(),
                |mut ts| black_box(ts.record_event(black_box(peer), event)),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
// (e) crc32_bench — crc32 for various data sizes
// ═══════════════════════════════════════════════════════════════

fn crc32_bench(c: &mut Criterion) {
    let sizes: &[usize] = &[64, 256, 1024, 4096];

    let mut group = c.benchmark_group("crc32");
    for &size in sizes {
        let data = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(format!("{}B", size), |b| {
            b.iter(|| black_box(neuron_wire::crc::crc32(black_box(&data))))
        });
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════
// (f) throughput_bench — full message pipeline
//    build_frame → sign → encrypt → parse_frame → verify → decrypt
// ═══════════════════════════════════════════════════════════════

fn throughput_bench(c: &mut Criterion) {
    use neuron_wire::header;
    use neuron_wire::identity::{
        verify_signature, NodeIdentity, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
    };
    use neuron_wire::secure_channel::SecureChannel;
    use neuron_wire::transport::TransportHeader;

    let alice_id = NodeIdentity::new();
    let bob_id = NodeIdentity::new();
    let mut alice_channel = SecureChannel::new();
    let mut bob_channel = SecureChannel::new();
    let alice_pub = alice_id.public_key_bytes();
    let bob_pub = bob_id.public_key_bytes();
    let session_a = alice_channel.handshake(&alice_id, bob_pub);
    let _session_b = bob_channel.handshake(&bob_id, alice_pub);

    let sizes: &[usize] = &[64, 256, 1024, 4096];

    let mut group = c.benchmark_group("throughput_pipeline");
    for &size in sizes {
        let body = vec![0x42u8; size];
        group.throughput(Throughput::Bytes(size as u64 + 16 + 96)); // body + header + auth prefix

        group.bench_function(format!("{}B", size), |b| {
            b.iter_batched(
                || body.clone(),
                |pt| {
                    // 1. Build NWP frame
                    let frame =
                        header::build_frame(black_box(20), black_box(pt.clone()), black_box(0));

                    // 2. Sign the frame body
                    let signature = alice_id.sign(black_box(&frame));
                    let sig_bytes = signature.to_bytes();

                    // 3. Encrypt the body through the secure channel
                    let (nonce, ciphertext) = alice_channel
                        .encrypt(&session_a, black_box(&frame), b"nwp")
                        .expect("encrypt");

                    // 4. Build transport header + prepend auth prefix
                    let t_header = TransportHeader::new(1, 0, 0, 42);
                    let t_bytes = t_header.to_bytes();
                    // Simulate wire packet: [transport_header][pubkey][signature][nonce][ciphertext]
                    let mut wire = Vec::with_capacity(
                        TransportHeader::SIZE
                            + PUBLIC_KEY_LENGTH
                            + SIGNATURE_LENGTH
                            + 16
                            + ciphertext.len(),
                    );
                    wire.extend_from_slice(&t_bytes);
                    wire.extend_from_slice(&alice_pub);
                    wire.extend_from_slice(&sig_bytes);
                    wire.extend_from_slice(&nonce);
                    wire.extend_from_slice(&ciphertext);

                    // 5. Parse: extract transport header
                    let parsed_t = unsafe { TransportHeader::from_bytes(black_box(&wire)) };
                    let offset = TransportHeader::SIZE;
                    let recv_pub: &[u8; PUBLIC_KEY_LENGTH] =
                        &wire[offset..offset + PUBLIC_KEY_LENGTH].try_into().unwrap();
                    let recv_sig: &[u8; SIGNATURE_LENGTH] = wire
                        [offset + PUBLIC_KEY_LENGTH..offset + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH]
                        .try_into()
                        .unwrap();
                    let recv_nonce: &[u8; 24] = wire[offset + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH
                        ..offset + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + 24]
                        .try_into()
                        .unwrap();
                    let recv_ct = &wire[offset + PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + 24..];

                    // 6. Verify signature
                    let _ = black_box(verify_signature(
                        black_box(recv_pub),
                        black_box(&frame),
                        black_box(recv_sig),
                    ));

                    // 7. Decrypt
                    let _ = black_box(bob_channel.decrypt(
                        black_box(&_session_b),
                        black_box(recv_nonce),
                        black_box(recv_ct),
                        b"nwp",
                    ));

                    black_box((parsed_t.sequence_number, ciphertext.len()));
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

// ═══════════════════════════════════════════════════════════════
// Criterion group + main
// ═══════════════════════════════════════════════════════════════

criterion_group!(
    benches,
    header_roundtrip,
    transport_header,
    crypto_bench,
    trust_bench,
    crc32_bench,
    throughput_bench,
);
criterion_main!(benches);
