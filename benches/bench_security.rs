//! Criterion benchmarks for neuron-wire security and transport hot paths.
//!
//! These benchmarks measure the critical-path operations that determine
//! whether the security layer bottlenecks the system:
//!
//! - Ed25519 signing/verification (authentication)
//! - XChaCha20-Poly1305 encrypt/decrypt (confidentiality)
//! - X25519 ECDH key agreement (forward secrecy)
//! - Secure channel handshake (session establishment)
//! - Trust system operations (Sybil resistance)
//! - Gradient weight calculation (transport)

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};

// ─── Ed25519 Signature Operations ──────────────────────────────

fn bench_ed25519_sign(c: &mut Criterion) {
    use neuron_wire::identity::NodeIdentity;
    let identity = NodeIdentity::generate();
    let message = vec![0x42u8; 256]; // typical gradient payload
    c.bench_function("ed25519_sign_256B", |b| {
        b.iter(|| black_box(identity.sign(black_box(&message))))
    });
}

fn bench_ed25519_verify(c: &mut Criterion) {
    use neuron_wire::identity::{NodeIdentity, entity_id_from_public_key, verify_signature};
    let identity = NodeIdentity::generate();
    let message = vec![0x42u8; 256];
    let signature = identity.sign(&message);
    let sig_bytes = signature.to_bytes();
    let pub_bytes = identity.public_key_bytes();
    c.bench_function("ed25519_verify_256B", |b| {
        b.iter(|| {
            black_box(verify_signature(
                black_box(&pub_bytes),
                black_box(&message),
                black_box(&sig_bytes),
            ))
        })
    });
}

fn bench_ed25519_sign_sizes(c: &mut Criterion) {
    use neuron_wire::identity::NodeIdentity;
    let identity = NodeIdentity::generate();
    for &size in &[64usize, 256, 1024, 4096] {
        let message = vec![0x42u8; size];
        c.bench_function(format!("ed25519_sign_{}B", size), |b| {
            b.iter(|| black_box(identity.sign(black_box(&message))))
        });
    }
}

// ─── XChaCha20-Poly1305 AEAD ──────────────────────────────────

fn bench_xchacha20_encrypt(c: &mut Criterion) {
    use chacha20poly1305::{XChaCha20Poly1305, KeyInit, aead::Aead};
    use rand::RngCore;
    let key = chacha20poly1305::Key::<XChaCha20Poly1305>::generate(&mut rand::thread_rng());
    let cipher = XChaCha20Poly1305::new(&key);
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
    let plaintext = vec![0x42u8; 256];
    c.bench_function("xchacha20poly1305_encrypt_256B", |b| {
        b.iter(|| black_box(cipher.encrypt(black_box(nonce), black_box(plaintext.as_slice()))))
    });
}

fn bench_xchacha20_decrypt(c: &mut Criterion) {
    use chacha20poly1305::{XChaCha20Poly1305, KeyInit, aead::Aead};
    use rand::RngCore;
    let key = chacha20poly1305::Key::<XChaCha20Poly1305>::generate(&mut rand::thread_rng());
    let cipher = XChaCha20Poly1305::new(&key);
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
    let plaintext = vec![0x42u8; 256];
    let ciphertext = cipher.encrypt(nonce, plaintext.as_slice()).unwrap();
    c.bench_function("xchacha20poly1305_decrypt_256B", |b| {
        b.iter(|| black_box(cipher.decrypt(black_box(nonce), black_box(ciphertext.as_slice()))))
    });
}

fn bench_xchacha20_sizes(c: &mut Criterion) {
    use chacha20poly1305::{XChaCha20Poly1305, KeyInit, aead::Aead};
    use rand::RngCore;
    let key = chacha20poly1305::Key::<XChaCha20Poly1305>::generate(&mut rand::thread_rng());
    let cipher = XChaCha20Poly1305::new(&key);
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
    for &size in &[64usize, 256, 1024, 4096] {
        let plaintext = vec![0x42u8; size];
        let mut group = c.benchmark_group(format!("xchacha20_{}B", size));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function("encrypt", |b| {
            b.iter(|| black_box(cipher.encrypt(black_box(nonce), black_box(plaintext.as_slice()))))
        });
        group.finish();
    }
}

// ─── X25519 ECDH Key Agreement ─────────────────────────────────

fn bench_x25519_ecdh(c: &mut Criterion) {
    use x25519_dalek::{StaticSecret, PublicKey};
    let alice_secret = StaticSecret::random_from_rng(&mut rand::thread_rng());
    let alice_public = PublicKey::from(&alice_secret);
    let bob_secret = StaticSecret::random_from_rng(&mut rand::thread_rng());
    let bob_public = PublicKey::from(&bob_secret);
    c.bench_function("x25519_ecdh", |b| {
        b.iter(|| black_box(alice_secret.diffie_hellman(black_box(&bob_public))))
    });
}

fn bench_x25519_keygen(c: &mut Criterion) {
    use x25519_dalek::{StaticSecret, PublicKey};
    c.bench_function("x25519_keygen", |b| {
        b.iter(|| {
            let secret = StaticSecret::random_from_rng(&mut rand::thread_rng());
            black_box(PublicKey::from(&secret))
        })
    });
}

// ─── SHA-256 Hashing ───────────────────────────────────────────

fn bench_sha256(c: &mut Criterion) {
    use sha2::{Sha256, Digest};
    for &size in &[32usize, 64, 256, 1024] {
        let data = vec![0xABu8; size];
        let mut group = c.benchmark_group(format!("sha256_{}B", size));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function("sha256", |b| {
            b.iter(|| {
                let mut hasher = Sha256::new();
                hasher.update(black_box(&data));
                black_box(hasher.finalize())
            })
        });
        group.finish();
    }
}

// ─── Secure Channel Handshake ──────────────────────────────────

fn bench_secure_channel_handshake(c: &mut Criterion) {
    use neuron_wire::secure_channel::SecureChannel;
    use neuron_wire::identity::NodeIdentity;
    let alice_identity = NodeIdentity::generate();
    let bob_identity = NodeIdentity::generate();
    c.bench_function("secure_channel_handshake", |b| {
        b.iter_batched(
            || {
                let mut alice = SecureChannel::new();
                let mut bob = SecureChannel::new();
                (alice, bob, alice_identity.clone(), bob_identity.clone())
            },
            |(mut alice, mut bob, alice_id, bob_id)| {
                let alice_pub = alice_id.public_key_bytes();
                let bob_pub = bob_id.public_key_bytes();
                let session_a = alice.handshake(&alice_id, bob_pub);
                let session_b = bob.handshake(&bob_id, alice_pub);
                black_box((session_a, session_b));
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_secure_channel_encrypt(c: &mut Criterion) {
    use neuron_wire::secure_channel::SecureChannel;
    use neuron_wire::identity::NodeIdentity;
    let alice_identity = NodeIdentity::generate();
    let bob_identity = NodeIdentity::generate();
    let mut alice = SecureChannel::new();
    let mut bob = SecureChannel::new();
    let alice_pub = alice_identity.public_key_bytes();
    let bob_pub = bob_identity.public_key_bytes();
    let session_a = alice.handshake(&alice_identity, bob_pub);
    let session_b = bob.handshake(&bob_identity, alice_pub);
    let plaintext = vec![0x42u8; 256];
    c.bench_function("secure_channel_encrypt_256B", |b| {
        b.iter(|| black_box(alice.encrypt(black_box(&session_a), black_box(&plaintext)).unwrap()))
    });
}

fn bench_secure_channel_decrypt(c: &mut Criterion) {
    use neuron_wire::secure_channel::SecureChannel;
    use neuron_wire::identity::NodeIdentity;
    let alice_identity = NodeIdentity::generate();
    let bob_identity = NodeIdentity::generate();
    let mut alice = SecureChannel::new();
    let mut bob = SecureChannel::new();
    let alice_pub = alice_identity.public_key_bytes();
    let bob_pub = bob_identity.public_key_bytes();
    let session_a = alice.handshake(&alice_identity, bob_pub);
    let session_b = bob.handshake(&bob_identity, alice_pub);
    let plaintext = vec![0x42u8; 256];
    let ciphertext = alice.encrypt(&session_a, &plaintext).unwrap();
    c.bench_function("secure_channel_decrypt_256B", |b| {
        b.iter(|| black_box(bob.decrypt(black_box(&session_b), black_box(&ciphertext)).unwrap()))
    });
}

// ─── Trust System ──────────────────────────────────────────────

fn bench_trust_record_event(c: &mut Criterion) {
    use neuron_wire::trust::{TrustSystem, TrustEvent};
    use neuron_wire::types::EntityId;
    let mut ts = TrustSystem::new();
    let eid = EntityId([1u8; 32]);
    c.bench_function("trust_record_event", |b| {
        b.iter(|| black_box(ts.record_event(black_box(eid), TrustEvent::ValidSignature)))
    });
}

fn bench_trust_check_rate_limit(c: &mut Criterion) {
    use neuron_wire::trust::TrustSystem;
    use neuron_wire::types::EntityId;
    let mut ts = TrustSystem::new();
    let eid = EntityId([1u8; 32]);
    // Build up some trust first
    for _ in 0..20 {
        ts.record_event(eid, neuron_wire::trust::TrustEvent::ValidSignature);
    }
    c.bench_function("trust_check_rate_limit", |b| {
        b.iter(|| black_box(ts.check_rate_limit(black_box(&eid))))
    });
}

fn bench_trust_cleanup(c: &mut Criterion) {
    use neuron_wire::trust::TrustSystem;
    use neuron_wire::types::EntityId;
    let mut ts = TrustSystem::new();
    for i in 0..1000 {
        let mut eid = [0u8; 32];
        eid[0..4].copy_from_slice(&(i as u32).to_le_bytes());
        ts.record_event(EntityId(eid), neuron_wire::trust::TrustEvent::ValidSignature);
    }
    c.bench_function("trust_cleanup_1000_peers", |b| {
        b.iter_batched(
            || ts.clone(),
            |mut ts| {
                black_box(ts.cleanup_expired());
            },
            BatchSize::SmallInput,
        )
    });
}

// ─── Gradient Weight Calculation ───────────────────────────────

fn bench_gradient_weight(c: &mut Criterion) {
    use neuron_wire::transport::calculate_gradient_weight;
    c.bench_function("gradient_weight", |b| {
        b.iter(|| black_box(calculate_gradient_weight(black_box(100.0), 50.0)))
    });
}

fn bench_gradient_weight_sizes(c: &mut Criterion) {
    use neuron_wire::transport::calculate_gradient_weight;
    for &half_life in &[10.0f32, 50.0, 100.0, 500.0] {
        c.bench_function(format!("gradient_weight_hl{:.0}", half_life), |b| {
            b.iter(|| black_box(calculate_gradient_weight(black_box(1000.0), half_life)))
        });
    }
}

// ─── Registration ──────────────────────────────────────────────

criterion_group!(
    benches,
    // Ed25519
    bench_ed25519_sign,
    bench_ed25519_verify,
    bench_ed25519_sign_sizes,
    // XChaCha20-Poly1305
    bench_xchacha20_encrypt,
    bench_xchacha20_decrypt,
    bench_xchacha20_sizes,
    // X25519 ECDH
    bench_x25519_ecdh,
    bench_x25519_keygen,
    // SHA-256
    bench_sha256,
    // Secure Channel
    bench_secure_channel_handshake,
    bench_secure_channel_encrypt,
    bench_secure_channel_decrypt,
    // Trust System
    bench_trust_record_event,
    bench_trust_check_rate_limit,
    bench_trust_cleanup,
    // Transport
    bench_gradient_weight,
    bench_gradient_weight_sizes,
);
criterion_main!(benches);
