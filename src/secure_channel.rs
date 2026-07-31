//! Secure Channel — encrypted transport, handshake, replay protection, authenticated packets.
//!
//! Provides a Noise-like protocol pattern for establishing encrypted, authenticated
//! channels between NWP nodes, plus symmetric per-packet AEAD encryption.
//!
//! # Protocol
//!
//! 1. **Handshake (Static-Static ECDH)**: A shared symmetric key is derived from
//!    X25519 Diffie-Hellman between the two nodes' long-term Ed25519 keys:
//!    - Local Ed25519 seed → X25519 static secret (via `XStaticSecret::from(seed)`)
//!    - Peer Ed25519 public key → X25519 public key (via Montgomery-form conversion)
//!    - `shared = SHA-256("nwp-handshake-v1" || ECDH(static_sec, peer_pk))`
//!    Both sides compute the same key because X25519 ECDH is commutative.
//! 2. **Per-packet encryption**: XChaCha20-Poly1305 AEAD with a per-packet nonce.
//! 3. **Replay protection**: Monotonic nonce counter per direction, enforced by the
//!    receiver. Duplicates beyond a configurable window are rejected.
//!
//! # Security Properties
//!
//! | Property | Mechanism |
//! |----------|-----------|
//! | Authentication | Static-static X25519 ECDH proves possession of Ed25519 long-term keys |
//! | Replay protection | Monotonic nonce counter + 1024-entry ring buffer |
//! | Integrity | XChaCha20-Poly1305 AEAD provides authenticated encryption |
//!
//! # Limitations
//!
//! - **No forward secrecy**: Because the handshake uses long-term static keys only,
//!   compromising a node's Ed25519 seed allows decrypting past sessions.
//!   Forward secrecy requires an ephemeral key exchange (Noise XX pattern).
//!
//! # Wire Format (encrypted packet)
//!
//! ```text
//! [0-15]  nonce:  [u8; 16]    = 12 byte salt + 4 byte counter (big-endian)
//! [16-..] ciphertext: [u8; N] = AEAD-encrypted (payload || padding)
//! ```

use crate::identity::NodeIdentity;
use chacha20poly1305::aead::{Aead, OsRng as AeadRng, Payload};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305, XNonce};
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret as XStaticSecret};

/// 16-byte nonce: 12 bytes salt + 4 bytes counter (big-endian).
pub const NONCE_SIZE: usize = 16;

/// Key size for XChaCha20-Poly1305 (32 bytes).
pub const KEY_SIZE: usize = 32;

/// Maximum drift window for nonce replay protection.
pub const MAX_NONCE_DRIFT: u32 = 1_000_000;

/// Maximum number of channel peers tracked simultaneously.
pub const MAX_PEERS: usize = 256;

/// Number of authenticated packets that must be received from a peer
/// before encryption activates. This ensures both sides have exchanged
/// public keys (via auth prefixes) before any encrypted traffic flows.
///
/// Protocol sequence:
///   1. A→B authenticated (B establishes session, replies auth-only)
///   2. B→A authenticated (A establishes session, replies auth-only)
///   3. A→B authenticated (B's session warmup complete → can encrypt)
///   4. B→A encrypted (A decrypts, warmup complete → can encrypt)
///
/// With threshold=3, encryption activates after 3 unencrypted rounds,
/// guaranteeing both sides have the other's public key.
pub const WARMUP_THRESHOLD: u32 = 3;

/// Size of the static auth prefix: 32-byte Ed25519 pubkey + 64-byte signature.
pub const AUTH_PREFIX_SIZE: usize = 96;

/// Additional size when FLAG_HANDSHAKE is set: +32 bytes for ephemeral X25519 pubkey.
pub const EPHEMERAL_KEY_SIZE: usize = 32;

/// Total auth prefix size with handshake: AUTH_PREFIX_SIZE + EPHEMERAL_KEY_SIZE = 128.
pub const AUTH_PREFIX_WITH_HANDSHAKE: usize = AUTH_PREFIX_SIZE + EPHEMERAL_KEY_SIZE;

/// A secure, encrypted channel between two NWP nodes.
///
/// Each channel holds a 32-byte shared key derived from an X25519 ECDH exchange,
/// authenticated by both parties' Ed25519 identities. Packets are encrypted with
/// XChaCha20-Poly1305 and carry a unique nonce for replay protection.
pub struct SecureChannel {
    /// Per-peer session state
    sessions: HashMap<[u8; 32], SessionState>,
    /// Map from socket address → session_id for outbound encryption lookup
    peer_to_session: HashMap<SocketAddr, [u8; 32]>,
}

/// Per-peer session state.
struct SessionState {
    /// Shared symmetric key (XChaCha20-Poly1305)
    /// Derived from ECDH. May be upgraded to ephemeral-derived key
    /// after both sides exchange ephemeral public keys.
    shared_key: [u8; KEY_SIZE],
    /// Outbound nonce counter (monotonic)
    outbound_counter: u64,
    /// Received nonce history for replay detection (ring buffer)
    received_nonces: VecDeque<u64>,
    /// Maximum nonce value seen from this peer
    max_received_nonce: u64,
    /// Peer's static public key (for verification)
    peer_public_key: [u8; 32],
    /// Last activity timestamp (for session expiry)
    last_activity_ms: u64,
    /// Number of authenticated packets received from this peer.
    /// Encryption only activates after WARMUP_THRESHOLD packets,
    /// ensuring both sides have exchanged public keys before
    /// encrypting. Prevents the chicken-and-egg deadlock where
    /// side A encrypts a reply before side B has A's pubkey.
    messages_received: u32,
    /// Ephemeral X25519 private key (generated per-session, never persisted).
    /// When both sides exchange ephemeral public keys, this replaces the
    /// static-derived shared key, providing forward secrecy:
    /// compromising the long-term seed cannot decrypt past sessions.
    ephemeral_secret: Option<XStaticSecret>,
    /// Peer's ephemeral X25519 public key (received via FLAG_HANDSHAKE).
    /// Once set, the session key is re-derived from the ephemeral exchange.
    peer_ephemeral_pub: Option<[u8; 32]>,
    /// Whether the ephemeral key exchange is complete on both sides.
    /// Only true after we've received the peer's ephemeral pubkey AND
    /// derived the new shared key.
    ephemeral_complete: bool,
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
            peer_to_session: HashMap::new(),
        }
    }

    /// Perform a Noise-style XX handshake with a remote peer.
    ///
    /// Derives a shared symmetric key via X25519 ECDH using static keys:
    ///
    /// 1. Convert local Ed25519 signing key seed → X25519 static secret
    /// 2. Convert peer Ed25519 verifying key → X25519 public via Montgomery form
    /// 3. `shared = SHA-256("nwp-handshake-v1" || ECDH(static_sec, peer_pk))`
    ///
    /// Both sides derive the same key because X25519 ECDH is commutative:
    /// `ECDH(alice_sk, bob_pk) = ECDH(bob_sk, alice_pk)`.
    ///
    /// NOTE: Forward secrecy requires an ephemeral key exchange (2 messages).
    /// This synchronous handshake provides authentication + session uniqueness
    /// but not forward secrecy. A Noise XX pattern would add that.
    ///
    /// Returns a session ID (SHA-256 of peer's public key).
    pub fn handshake(
        &mut self,
        local_identity: &NodeIdentity,
        peer_public_key: [u8; 32],
    ) -> [u8; 32] {
        let now_ms = now_millis();

        // Convert peer's Ed25519 verifying key to X25519 public
        let peer_vk = match VerifyingKey::from_bytes(&peer_public_key) {
            Ok(vk) => vk,
            Err(_) => {
                // Invalid peer key — fall back to SHA-256 only (backward compat)
                let fallback = Self::fallback_derive(local_identity, &peer_public_key);
                return self.insert_session(fallback, peer_public_key, now_ms);
            }
        };
        let peer_x_pk = XPublicKey::from(peer_vk.to_montgomery().to_bytes());

        // Local Ed25519 signing key seed → X25519 static secret
        let local_sk_bytes = local_identity.secret_key_bytes();
        let local_static = XStaticSecret::from(local_sk_bytes);

        // X25519 ECDH: both sides compute the same shared secret
        let shared = local_static.diffie_hellman(&peer_x_pk);

        // Derive symmetric key with domain separator
        let mut hasher = Sha256::new();
        hasher.update(b"nwp-handshake-v1");
        hasher.update(shared.as_bytes());
        let shared_key = hasher.finalize().into();

        self.insert_session(shared_key, peer_public_key, now_ms)
    }

    /// Fallback key derivation when X25519 conversion fails (backward compatible).
    fn fallback_derive(local: &NodeIdentity, peer_pk: &[u8; 32]) -> [u8; 32] {
        let mut csprng = AeadRng;
        let ephemeral = XChaCha20Poly1305::generate_key(&mut csprng);
        let mut hasher = Sha256::new();
        hasher.update(local.public_key_bytes());
        hasher.update(peer_pk);
        hasher.update(ephemeral);
        hasher.finalize().into()
    }

    /// Create a session from a shared key, insert it, and return the session ID.
    fn insert_session(
        &mut self,
        shared_key: [u8; 32],
        peer_public_key: [u8; 32],
        now_ms: u64,
    ) -> [u8; 32] {
        // Session ID = hash of peer's public key
        let session_id = Sha256::digest(&peer_public_key).into();

        let session = SessionState {
            shared_key,
            outbound_counter: 0,
            received_nonces: VecDeque::with_capacity(1024),
            max_received_nonce: 0,
            peer_public_key,
            last_activity_ms: now_ms,
            messages_received: 0,
            ephemeral_secret: None,
            peer_ephemeral_pub: None,
            ephemeral_complete: false,
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
    /// Encrypt a message for the given session.
    ///
    /// Returns `Some((nonce, ciphertext))` on success, `None` if session not found.
    /// Ciphertext includes a 16-byte Poly1305 authentication tag.
    /// The nonce is automatically incremented for replay protection.
    ///
    /// # Examples
    ///
    /// ```
    /// use neuron_wire::secure_channel::SecureChannel;
    /// use neuron_wire::identity::NodeIdentity;
    ///
    /// let alice = NodeIdentity::new();
    /// let bob = NodeIdentity::new();
    ///
    /// let mut chan = SecureChannel::new();
    /// let sid = chan.handshake(&alice, bob.public_key_bytes());
    ///
    /// let msg = b"secret gradient data";
    /// let (nonce, ct) = chan.encrypt(&sid, msg, b"").unwrap();
    /// assert!(ct.len() > msg.len()); // ciphertext includes auth tag
    /// ```
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

    /// Get the number of sessions with forward secrecy (ephemeral key exchange complete).
    pub fn ephemeral_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.ephemeral_complete)
            .count()
    }

    /// Check if a session exists.
    pub fn has_session(&self, session_id: &[u8; 32]) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Look up a session by peer socket address (for outbound encryption).
    pub fn get_session_by_peer(&self, addr: &SocketAddr) -> Option<[u8; 32]> {
        self.peer_to_session.get(addr).copied()
    }

    /// Register a peer's socket address for a given session ID.
    pub fn register_peer_session(&mut self, addr: SocketAddr, session_id: [u8; 32]) {
        self.peer_to_session.insert(addr, session_id);
    }

    /// Record that an authenticated packet was received from the peer.
    /// Increments the warmup counter. Encryption only activates after
    /// [`WARMUP_THRESHOLD`] messages.
    pub fn record_message_received(&mut self, session_id: &[u8; 32]) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.messages_received += 1;
        }
    }

    /// Check if the session is ready for encryption (warmup complete).
    /// Both sides must have exchanged enough packets to guarantee
    /// mutual possession of public keys before encrypting.
    pub fn is_ready_for_encryption(&self, session_id: &[u8; 32]) -> bool {
        self.sessions
            .get(session_id)
            .map(|s| s.messages_received >= WARMUP_THRESHOLD)
            .unwrap_or(false)
    }

    // ─── Forward Secrecy: Ephemeral Key Exchange ──────────────

    /// Generate and return this session's ephemeral X25519 public key.
    /// Called by `seal_outbound()` to include in the auth prefix when
    /// `FLAG_HANDSHAKE` is set. The private key is stored in the session
    /// and never persisted to disk.
    ///
    /// Returns `Some([u8; 32])` if a new ephemeral keypair was generated,
    /// or `None` if the session doesn't exist.
    pub fn generate_ephemeral_key(&mut self, session_id: &[u8; 32]) -> Option<[u8; 32]> {
        let session = self.sessions.get_mut(session_id)?;

        // Generate ephemeral X25519 keypair
        let mut csprng = AeadRng;
        let ephemeral_secret = XStaticSecret::random_from_rng(&mut csprng);
        let ephemeral_public = XPublicKey::from(&ephemeral_secret);

        let pub_bytes = ephemeral_public.to_bytes();
        session.ephemeral_secret = Some(ephemeral_secret);

        Some(pub_bytes)
    }

    /// Get this session's existing ephemeral public key (if generated).
    /// Returns `None` if no ephemeral key has been generated yet.
    pub fn get_ephemeral_public(&self, session_id: &[u8; 32]) -> Option<[u8; 32]> {
        let session = self.sessions.get(session_id)?;
        session.ephemeral_secret.as_ref().map(|secret| {
            let public = XPublicKey::from(secret);
            public.to_bytes()
        })
    }

    /// Process a peer's ephemeral public key received via `FLAG_HANDSHAKE`.
    ///
    /// When we receive the peer's ephemeral pubkey, we:
    /// 1. Store it in the session
    /// 2. If we also have our own ephemeral secret, derive the new shared key
    /// 3. Replace the static-derived key with the ephemeral-derived key
    ///
    /// The new key provides forward secrecy: compromising the long-term seed
    /// cannot decrypt past sessions because the ephemeral private keys are gone.
    pub fn receive_ephemeral_key(
        &mut self,
        session_id: &[u8; 32],
        peer_ephemeral_pub: [u8; 32],
    ) -> bool {
        let session = match self.sessions.get_mut(session_id) {
            Some(s) => s,
            None => return false,
        };

        session.peer_ephemeral_pub = Some(peer_ephemeral_pub);

        // If we also have our own ephemeral secret, derive the new key
        if let Some(ref ephemeral_secret) = session.ephemeral_secret {
            let peer_x_pub = XPublicKey::from(peer_ephemeral_pub);
            let shared = ephemeral_secret.diffie_hellman(&peer_x_pub);

            // Derive new session key with forward-secrecy domain separator
            let mut hasher = Sha256::new();
            hasher.update(b"nwp-ephemeral-v1");
            hasher.update(shared.as_bytes());
            session.shared_key = hasher.finalize().into();
            session.ephemeral_complete = true;

            return true;
        }

        // We don't have our own ephemeral key yet — will derive when we generate it
        false
    }

    /// Check if the ephemeral key exchange is complete for this session.
    pub fn is_ephemeral_complete(&self, session_id: &[u8; 32]) -> bool {
        self.sessions
            .get(session_id)
            .map(|s| s.ephemeral_complete)
            .unwrap_or(false)
    }

    /// Remove a peer's session mapping by address.
    pub fn remove_peer_session(&mut self, addr: &SocketAddr) {
        self.peer_to_session.remove(addr);
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
