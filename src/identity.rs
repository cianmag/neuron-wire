//! Node Identity — Ed25519 keypairs, digital signatures, EntityId derivation.
//!
//! Every node in the NWP network has a cryptographic identity backed by an
//! Ed25519 keypair. The public key is used to derive the node's EntityId
//! (256-bit), providing cryptographically-bound identity that prevents
//! impersonation and Sybil attacks.
//!
//! # Security Properties
//!
//! | Property | Mechanism |
//! |----------|-----------|
//! | Non-repudiation | Ed25519 signatures on every outbound packet |
//! | Identity binding | EntityId = SHA-256(public_key), verified by signature |
//! | Forward secrecy | Ephemeral session keys rotated per handshake |
//! | Key rotation | `rotate()` creates fresh identity, verifiable via cert chain |
//!
//! # Wire Format (96-byte auth prefix on packets)
//!
//! ```text
//! [0-31]   public_key: [u8; 32]    = Ed25519 verifying key
//! [32-95]  signature:  [u8; 64]    = Ed25519 over (seq || timestamp || body_hash)
//! ```

use ed25519_dalek::{
    Signature, SignatureError, Signer, SigningKey, VerifyingKey, SECRET_KEY_LENGTH,
};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::fmt;

use crate::components::EntityId;

/// Length of an Ed25519 public key in bytes (32).
pub const PUBLIC_KEY_LENGTH: usize = 32;

/// Length of an Ed25519 signature in bytes (64).
pub const SIGNATURE_LENGTH: usize = 64;

/// Length of the signed auth prefix: 32 bytes public key + 64 bytes signature.
pub const AUTH_PREFIX_LENGTH: usize = PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH;

/// A cryptographic node identity backed by Ed25519.
///
/// Contains both the signing key (secret) and the verifying key (public).
/// The `EntityId` is deterministically derived from the public key.
pub struct NodeIdentity {
    /// Ed25519 signing key (SECRET)
    signing_key: SigningKey,
    /// Ed25519 verifying key (PUBLIC)
    verifying_key: VerifyingKey,
    /// Derived EntityId = SHA-256(verifying_key)
    entity_id: EntityId,
    /// Monotonic sequence number for outbound packets
    sequence_number: u64,
}

impl fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeIdentity")
            .field("entity_id", &self.entity_id)
            .field("sequence_number", &self.sequence_number)
            .finish()
    }
}

impl NodeIdentity {
    /// Generate a fresh node identity with a random Ed25519 keypair.
    ///
    /// Uses the OS entropy source (e.g., `/dev/urandom`).
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let entity_id = Self::derive_entity_id(&verifying_key);

        NodeIdentity {
            signing_key,
            verifying_key,
            entity_id,
            sequence_number: 0,
        }
    }

    /// Create a node identity from an existing Ed25519 seed (32 bytes).
    ///
    /// Useful for deterministic identities (testing, reproducible nodes).
    pub fn from_seed(seed: &[u8; SECRET_KEY_LENGTH]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        let entity_id = Self::derive_entity_id(&verifying_key);

        NodeIdentity {
            signing_key,
            verifying_key,
            entity_id,
            sequence_number: 0,
        }
    }

    /// Create a node identity from a full keypair (64 bytes: secret || public).
    ///
    /// Useful for loading persisted identities.
    pub fn from_keypair_bytes(bytes: &[u8; 64]) -> Result<Self, SignatureError> {
        let signing_key = SigningKey::from_keypair_bytes(bytes)?;
        let verifying_key = signing_key.verifying_key();
        let entity_id = Self::derive_entity_id(&verifying_key);

        Ok(NodeIdentity {
            signing_key,
            verifying_key,
            entity_id,
            sequence_number: 0,
        })
    }

    /// Derive an EntityId from an Ed25519 verifying key via SHA-256.
    ///
    /// EntityId = SHA-256(public_key_bytes)
    /// This provides collision resistance and binds identity to the key.
    fn derive_entity_id(vk: &VerifyingKey) -> EntityId {
        let hash = Sha256::digest(vk.as_bytes());
        let mut id = [0u8; 32];
        id.copy_from_slice(&hash);
        EntityId(id)
    }

    /// Get the node's EntityId (derived from the public key).
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    /// Get the Ed25519 verifying key (public key).
    pub fn public_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Get the public key bytes (32 bytes).
    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        *self.verifying_key.as_bytes()
    }

    /// Get the secret key bytes (32 bytes).
    pub fn secret_key_bytes(&self) -> [u8; SECRET_KEY_LENGTH] {
        self.signing_key.to_bytes()
    }

    /// Sign a message with this identity's signing key.
    ///
    /// Returns a 64-byte Ed25519 signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Sign a packet payload and return the 64-byte Ed25519 signature.
    ///
    /// The signed data covers: sequence_number (8 bytes) || timestamp (8 bytes) || body_hash (32 bytes).
    /// This binds the signature to a specific packet context, preventing replay attacks.
    pub fn sign_packet(&self, seq: u64, timestamp_ms: u64, body_hash: &[u8; 32]) -> Signature {
        let mut data = Vec::with_capacity(8 + 8 + 32);
        data.extend_from_slice(&seq.to_le_bytes());
        data.extend_from_slice(&timestamp_ms.to_le_bytes());
        data.extend_from_slice(body_hash);
        self.sign(&data)
    }

    /// Get the next sequence number and advance the counter.
    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.sequence_number;
        self.sequence_number += 1;
        seq
    }

    /// Rotate to a fresh identity keypair.
    ///
    /// Returns the old verifying key for certificate chain validation.
    pub fn rotate(&mut self) -> VerifyingKey {
        let old_vk = self.verifying_key;
        let mut csprng = OsRng;
        let new_sk = SigningKey::generate(&mut csprng);
        let new_vk = new_sk.verifying_key();
        let new_eid = Self::derive_entity_id(&new_vk);

        // Replace (old key is dropped, memory overwritten by Rust's move semantics)
        self.signing_key = new_sk;
        self.verifying_key = new_vk;
        self.entity_id = new_eid;
        self.sequence_number = 0;

        old_vk
    }
}

impl Default for NodeIdentity {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Static verification functions ────────────────────────────

/// Verify an Ed25519 signature against a public key and message.
///
/// Returns `Ok(())` if the signature is valid, `Err(SignatureError)` otherwise.
pub fn verify_signature(
    public_key: &[u8; PUBLIC_KEY_LENGTH],
    message: &[u8],
    signature: &[u8; SIGNATURE_LENGTH],
) -> Result<(), SignatureError> {
    let vk = VerifyingKey::from_bytes(public_key)?;
    let sig = Signature::from_bytes(signature);
    vk.verify_strict(message, &sig)
}

/// Verify a packet signature.
///
/// The signed data is reconstructed as:
/// seq (8 bytes LE) || timestamp_ms (8 bytes LE) || body_hash (32 bytes).
pub fn verify_packet_signature(
    public_key: &[u8; PUBLIC_KEY_LENGTH],
    seq: u64,
    timestamp_ms: u64,
    body_hash: &[u8; 32],
    signature: &[u8; SIGNATURE_LENGTH],
) -> Result<(), SignatureError> {
    let mut data = Vec::with_capacity(8 + 8 + 32);
    data.extend_from_slice(&seq.to_le_bytes());
    data.extend_from_slice(&timestamp_ms.to_le_bytes());
    data.extend_from_slice(body_hash);
    verify_signature(public_key, &data, signature)
}

/// Derive an EntityId from a public key (SHA-256 hash).
pub fn entity_id_from_public_key(public_key: &[u8; PUBLIC_KEY_LENGTH]) -> EntityId {
    let hash = Sha256::digest(public_key);
    let mut id = [0u8; 32];
    id.copy_from_slice(&hash);
    EntityId(id)
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_creation() {
        let id = NodeIdentity::new();
        let eid = id.entity_id();
        let pk = id.public_key_bytes();
        let derived = entity_id_from_public_key(&pk);
        assert_eq!(eid, derived, "EntityId must derive from public key");
    }

    #[test]
    fn test_sign_and_verify() {
        let id = NodeIdentity::new();
        let message = b"hello world";
        let signature = id.sign(message);
        assert!(
            id.public_key().verify_strict(message, &signature).is_ok(),
            "signature must verify"
        );
    }

    #[test]
    fn test_sign_and_verify_bytes() {
        let id = NodeIdentity::new();
        let message = b"test packet data";
        let signature = id.sign(message);
        let sig_bytes: [u8; SIGNATURE_LENGTH] = signature.to_bytes();
        let pk_bytes = id.public_key_bytes();

        let result = verify_signature(&pk_bytes, message, &sig_bytes);
        assert!(result.is_ok(), "verification via bytes must succeed");
    }

    #[test]
    fn test_packet_signature() {
        let id = NodeIdentity::new();
        let body_hash = Sha256::digest(b"payload data").into();
        let sig = id.sign_packet(42, 1_234_567_890, &body_hash);

        let sig_bytes: [u8; SIGNATURE_LENGTH] = sig.to_bytes();
        let pk_bytes = id.public_key_bytes();

        let result = verify_packet_signature(&pk_bytes, 42, 1_234_567_890, &body_hash, &sig_bytes);
        assert!(result.is_ok(), "packet signature must verify");

        // Wrong seq should fail
        let wrong = verify_packet_signature(&pk_bytes, 43, 1_234_567_890, &body_hash, &sig_bytes);
        assert!(wrong.is_err(), "wrong seq must fail verification");

        // Wrong body hash should fail
        let wrong_hash = Sha256::digest(b"tampered").into();
        let wrong = verify_packet_signature(&pk_bytes, 42, 1_234_567_890, &wrong_hash, &sig_bytes);
        assert!(wrong.is_err(), "tampered hash must fail verification");
    }

    #[test]
    fn test_deterministic_seed() {
        let seed = [0xABu8; 32];
        let id1 = NodeIdentity::from_seed(&seed);
        let id2 = NodeIdentity::from_seed(&seed);
        assert_eq!(
            id1.entity_id(),
            id2.entity_id(),
            "same seed must produce same identity"
        );
        assert_eq!(
            id1.public_key_bytes(),
            id2.public_key_bytes(),
            "same seed must produce same public key"
        );
    }

    #[test]
    fn test_sequence_number() {
        let mut id = NodeIdentity::new();
        assert_eq!(id.next_sequence(), 0);
        assert_eq!(id.next_sequence(), 1);
        assert_eq!(id.next_sequence(), 2);
        assert_eq!(id.next_sequence(), 3);
    }

    #[test]
    fn test_identity_rotate() {
        let mut id = NodeIdentity::new();
        let old_eid = id.entity_id();
        let old_pk = id.public_key_bytes();

        let _old_vk = id.rotate();
        let new_pk = id.public_key_bytes();

        assert_ne!(old_eid, id.entity_id(), "rotation must change EntityId");
        assert_ne!(old_pk, new_pk, "rotation must change public key");
        assert_eq!(id.next_sequence(), 0, "rotation must reset sequence");
    }
}
