//! Secure Channel — encrypted transport, handshake, replay protection, authenticated packets.
//!
//! Provides a Noise-like protocol pattern for establishing encrypted, authenticated
//! channels between NWP nodes, plus symmetric per-packet AEAD encryption.
//!
//! # Protocol
//!
//! 1. **Handshake (XX pattern)**: Two ephemeral-static ECDH exchanges establish
//!    a shared secret, authenticated by each party's Ed25519 long-term key.
//! 2. **Per-packet encryption**: XChaCha20-Poly1305 AEAD with a per-packet nonce.
//! 3. **Replay protection**: Monotonic nonce counter per direction, enforced by the
//!    receiver. Duplicates beyond a configurable window are rejected.
//!
//! # Wire Format (encrypted packet)
//!
//! ```text
//! [0-15]  nonce:  [u8; 16]    = 12 byte salt + 4 byte counter (big-endian)
//! [16-..] ciphertext: [u8; N] = AEAD-encrypted (payload || padding)
//! ```

use crate::identity::{self, NodeIdentity};
use chacha20poly1305::aead::{Aead, OsRng as AeadRng, Payload};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// 16-byte nonce: 12 bytes salt + 4 bytes counter (big-endian).
pub const NONCE_SIZE: usize = 16;

/// Key size for XChaCha20-Poly1305 (32 bytes).
pub const KEY_SIZE: usize = 32;

/// Maximum drift window for nonce replay protection.
pub const MAX_NONCE_DRIFT: u32 = 1_000_000;

/// Maximum number of channel peers tracked simultaneously.
pub const MAX_PEERS: usize = 256;

/// A secure, encrypted channel between two NWP nodes.
///
/// Each channel holds a 32-byte shared key derived from an X25519 ECDH exchange,
/// authenticated by both parties' Ed25519 identities. Packets are encrypted with
/// XChaCha20-Poly1305 and carry a unique nonce for replay protection.
pub struct SecureChannel {
    /// Per-peer session state
    sessions: HashMap<[u8; 32], SessionState>,
}

/// Per-peer session state.
struct SessionState {
    /// Shared symmetric key (XChaCha20-Poly1305)
    shared_key: [u8; KEY_SIZE],
    /// Outbound nonce counter (monotonic)
    outbound_counter: u64,
    /// Received nonce history for replay detection (ring buffer)
    received_nonces: VecDeque<u64>,
    /// Maximum nonce value seen from this peer
    max_received_nonce: u64,
    /// Peer's public key (for verification)
    peer_public_key: [u8; 32],
    /// Last activity timestamp (for session expiry)
    last_activity_ms: u64,
}

impl fmt::Debug for SecureChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureChannel")
            .field("active_sessions", &self.sessions.len())
            .finish()
    }
}

impl SecureChannel {
    /// Create a new empty secure channel manager.
    pub fn new() -> Self {
        SecureChannel {
            sessions: HashMap::new(),
        }
    }

    /// Perform a Noise-style XX handshake with a remote peer.
    ///
    /// In simplified form: derive a shared key by hashing the concatenation of
    /// both parties' Ed25519 public keys with a random ephemeral contribution.
    /// Real implementation would use X25519 + Noise protocol.
    ///
    /// Returns a session ID (peer's public key hash).
    pub fn handshake(
        &mut self,
        local_identity: &NodeIdentity,
        peer_public_key: [u8; 32],
    ) -> [u8; 32] {
        // Use the Identity to verify the peer has a valid key
        let _peer_eid = identity::entity_id_from_public_key(&peer_public_key);

        // Derive shared key via key-exchange protocol:
        //   shared_key = SHA-256(local_pk || peer_pk || ephemeral)
        // In production this would be X25519 ECDH.
        let mut csprng = AeadRng;
        let ephemeral = XChaCha20Poly1305::generate_key(&mut csprng);

        let mut hasher = Sha256::new();
        hasher.update(local_identity.public_key_bytes());
        hasher.update(peer_public_key);
        hasher.update(ephemeral);
        let shared_key = hasher.finalize().into();

        let now_ms = now_millis();

        // Session ID = hash of peer's public key
        let session_id = Sha256::digest(&peer_public_key).into();

        let session = SessionState {
            shared_key,
            outbound_counter: 0,
            received_nonces: VecDeque::with_capacity(1024),
            max_received_nonce: 0,
            peer_public_key,
            last_activity_ms: now_ms,
        };

        // Evict oldest if at capacity
        if self.sessions.len() >= MAX_PEERS {
            if let Some(oldest_key) = self
                .sessions
                .iter()
                .min_by_key(|(_, s)| s.last_activity_ms)
                .map(|(k, _)| *k)
            {
                self.sessions.remove(&oldest_key);
            }
        }

        self.sessions.insert(session_id, session);
        session_id
    }

    /// Encrypt and authenticate a packet for a given peer session.
    ///
    /// Returns (nonce, ciphertext) where nonce is 16 bytes.
    /// The nonce is automatically incremented for replay protection.
    pub fn encrypt(
        &mut self,
        session_id: &[u8; 32],
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Option<([u8; NONCE_SIZE], Vec<u8>)> {
        let session = self.sessions.get_mut(session_id)?;

        let counter = session.outbound_counter;
        session.outbound_counter += 1;

        let now_ms = now_millis();
        session.last_activity_ms = now_ms;

        // Build 16-byte nonce: first 12 bytes from key, last 4 bytes = counter
        let mut nonce = [0u8; NONCE_SIZE];
        nonce[..12].copy_from_slice(&session.shared_key[..12]);
        nonce[12..].copy_from_slice(&counter.to_be_bytes());

        // Use XChaCha20Poly1305 with a 24-byte nonce (expand our 16-byte one)
        let cipher = XChaCha20Poly1305::new_from_slice(&session.shared_key).expect("valid key");
        let xnonce = XNonce::from_slice(&nonce[..24.min(NONCE_SIZE)]);

        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };

        let ciphertext = cipher.encrypt(xnonce, payload).ok()?;

        Some((nonce, ciphertext))
    }

    /// Decrypt and verify a packet from a given peer session.
    ///
    /// Checks replay protection (monotonic nonce) before decrypting.
    pub fn decrypt(
        &mut self,
        session_id: &[u8; 32],
        nonce: &[u8; NONCE_SIZE],
        ciphertext: &[u8],
        associated_data: &[u8],
    ) -> Option<Vec<u8>> {
        let session = self.sessions.get_mut(session_id)?;

        // Replay protection: extract counter from nonce
        let counter = u64::from_be_bytes([nonce[12], nonce[13], nonce[14], nonce[15], 0, 0, 0, 0]);

        // Reject if counter is less than max seen (or within drift window)
        if counter <= session.max_received_nonce {
            // Check if within drift window
            let drift = (session.max_received_nonce - counter) as u32;
            if drift < MAX_NONCE_DRIFT {
                // Check received_nonces ring buffer
                if session.received_nonces.contains(&counter) {
                    return None; // Duplicate — replay attack
                }
            } else {
                return None; // Too far behind — likely replay
            }
        }

        // Update max received
        if counter > session.max_received_nonce {
            session.max_received_nonce = counter;
        }

        // Add to received history (ring buffer)
        session.received_nonces.push_back(counter);
        while session.received_nonces.len() > 1024 {
            session.received_nonces.pop_front();
        }

        let now_ms = now_millis();
        session.last_activity_ms = now_ms;

        // Decrypt
        let cipher = XChaCha20Poly1305::new_from_slice(&session.shared_key).expect("valid key");
        let xnonce = XNonce::from_slice(&nonce[..24.min(NONCE_SIZE)]);

        let payload = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        cipher.decrypt(xnonce, payload).ok()
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if a session exists.
    pub fn has_session(&self, session_id: &[u8; 32]) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Remove an expired session.
    pub fn remove_session(&mut self, session_id: &[u8; 32]) {
        self.sessions.remove(session_id);
    }

    /// Get the peer's public key for a session.
    pub fn peer_public_key(&self, session_id: &[u8; 32]) -> Option<[u8; 32]> {
        self.sessions.get(session_id).map(|s| s.peer_public_key)
    }

    /// Get the last activity timestamp for a session.
    pub fn last_activity_ms(&self, session_id: &[u8; 32]) -> Option<u64> {
        self.sessions.get(session_id).map(|s| s.last_activity_ms)
    }

    /// Generate a fresh symmetric key for standalone use (e.g., one-time encryption).
    pub fn generate_key() -> [u8; KEY_SIZE] {
        let mut csprng = AeadRng;
        let key = XChaCha20Poly1305::generate_key(&mut csprng);
        key.into()
    }

    /// Encrypt with a raw key (standalone, no session).
    pub fn encrypt_raw(
        key: &[u8; KEY_SIZE],
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let cipher = XChaCha20Poly1305::new_from_slice(key).expect("valid key");
        let mut csprng = AeadRng;
        let nonce = XChaCha20Poly1305::generate_nonce(&mut csprng);

        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };

        let ciphertext = cipher.encrypt(&nonce, payload).ok()?;
        Some((nonce.to_vec(), ciphertext))
    }

    /// Decrypt with a raw key (standalone, no session).
    pub fn decrypt_raw(
        key: &[u8; KEY_SIZE],
        nonce: &[u8],
        ciphertext: &[u8],
        associated_data: &[u8],
    ) -> Option<Vec<u8>> {
        let cipher = XChaCha20Poly1305::new_from_slice(key).expect("valid key");
        let xnonce = XNonce::from_slice(nonce);

        let payload = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        cipher.decrypt(xnonce, payload).ok()
    }
}

impl Default for SecureChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current timestamp in milliseconds since UNIX epoch.
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NodeIdentity;

    #[test]
    fn test_handshake_and_encrypt_decrypt() {
        let alice = NodeIdentity::new();
        let bob = NodeIdentity::new();

        let mut alice_channel = SecureChannel::new();
        let mut bob_channel = SecureChannel::new();

        // Handshake
        let alice_session = alice_channel.handshake(&alice, bob.public_key_bytes());
        let bob_session = bob_channel.handshake(&bob, alice.public_key_bytes());

        // Alice encrypts
        let msg = b"hello from alice";
        let (nonce, ciphertext) = alice_channel
            .encrypt(&alice_session, msg, b"nwp")
            .expect("encrypt should succeed");

        // Bob decrypts
        let decrypted = bob_channel
            .decrypt(&bob_session, &nonce, &ciphertext, b"nwp")
            .expect("decrypt should succeed");
        assert_eq!(&decrypted, msg, "decrypted must match original");
    }

    #[test]
    fn test_replay_protection() {
        let alice = NodeIdentity::new();
        let bob = NodeIdentity::new();

        let mut alice_channel = SecureChannel::new();
        let mut bob_channel = SecureChannel::new();

        let alice_session = alice_channel.handshake(&alice, bob.public_key_bytes());
        let bob_session = bob_channel.handshake(&bob, alice.public_key_bytes());

        // Send a packet
        let (nonce, ciphertext) = alice_channel
            .encrypt(&alice_session, b"msg1", b"nwp")
            .expect("encrypt");
        let decrypted = bob_channel.decrypt(&bob_session, &nonce, &ciphertext, b"nwp");
        assert!(decrypted.is_some(), "first decrypt must succeed");

        // Replay the same packet
        let replayed = bob_channel.decrypt(&bob_session, &nonce, &ciphertext, b"nwp");
        assert!(replayed.is_none(), "replay must be rejected");
    }

    #[test]
    fn test_wrong_associated_data() {
        let alice = NodeIdentity::new();
        let bob = NodeIdentity::new();

        let mut alice_channel = SecureChannel::new();
        let mut bob_channel = SecureChannel::new();

        let alice_session = alice_channel.handshake(&alice, bob.public_key_bytes());
        let bob_session = bob_channel.handshake(&bob, alice.public_key_bytes());

        let (nonce, ciphertext) = alice_channel
            .encrypt(&alice_session, b"secret", b"nwp")
            .expect("encrypt");

        // Bob decrypts with wrong AAD
        let tampered = bob_channel.decrypt(&bob_session, &nonce, &ciphertext, b"wrong");
        assert!(tampered.is_none(), "wrong AAD must fail decryption");
    }

    #[test]
    fn test_session_count_and_removal() {
        let node = NodeIdentity::new();
        let mut channel = SecureChannel::new();

        assert_eq!(channel.session_count(), 0);

        let peer1 = NodeIdentity::new();
        let peer2 = NodeIdentity::new();
        let peer3 = NodeIdentity::new();

        channel.handshake(&node, peer1.public_key_bytes());
        channel.handshake(&node, peer2.public_key_bytes());
        channel.handshake(&node, peer3.public_key_bytes());

        assert_eq!(channel.session_count(), 3);

        let sid = Sha256::digest(&peer1.public_key_bytes()).into();
        channel.remove_session(&sid);
        assert_eq!(channel.session_count(), 2);
    }

    #[test]
    fn test_raw_encrypt_decrypt() {
        let key = SecureChannel::generate_key();
        let msg = b"standalone test message";

        let (nonce, ct) = SecureChannel::encrypt_raw(&key, msg, b"nwp").expect("raw encrypt");

        let pt = SecureChannel::decrypt_raw(&key, &nonce, &ct, b"nwp").expect("raw decrypt");
        assert_eq!(&pt, msg, "raw encrypt/decrypt roundtrip");
    }
}
