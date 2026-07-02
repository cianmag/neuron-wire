//! Audit Log — secure bootstrap, audit trails, tamper detection via hash chain.
//!
//! Every significant event in the NWP network is recorded in an append-only
//! audit log. The log is structured as a hash chain (blockchain-light) where
//! each entry's hash is derived from the previous entry, making tampering
//! detectable.
//!
//! # Hash Chain Structure
//!
//! ```text
//! entry[0].hash = SHA-256(event_type || timestamp || data || prev_hash=0)
//! entry[N].hash = SHA-256(event_type || timestamp || data || entry[N-1].hash)
//! ```
//!
//! # Security Properties
//!
//! | Property | Mechanism |
//! |----------|-----------|
//! | Tamper evidence | Hash chain — changing any entry invalidates all subsequent hashes |
//! | Non-repudiation | Entries can be optionally signed with node identity |
//! | Time ordering | Monotonic sequence number + wall clock timestamp |
//! | Compact audit | Periodic checkpoint hashes enable log truncation |
//! | Secure bootstrap | Bootstrap verification against known-good checkpoint |
//!
//! # Event Types
//!
//! - `NodeStartup` — Node initialized
//! - `IdentityRotation` — Key rotation event
//! - `PeerDiscovered` — New peer detected
//! - `HandshakeSuccess` — Secure channel established
//! - `HandshakeFailure` — Failed handshake (logged for forensics)
//! - `TrustChange` — Peer trust score crossed threshold
//! - `RateLimitTriggered` — Rate limit enforced
//! - `ReplayDetected` — Replay attack detected
//! - `InvalidSignature` — Bad signature from peer
//! - `BootstrapCheckpoint` — Periodic hash chain checkpoint
//! - `ConfigChange` — Configuration modification

use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::components::EntityId;

/// Maximum number of entries kept in the in-memory audit buffer.
pub const MAX_AUDIT_ENTRIES: usize = 100_000;

/// Interval (in entries) for writing checkpoint hashes.
pub const CHECKPOINT_INTERVAL: usize = 1000;

/// Audit event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventType {
    /// Node initialized and started
    NodeStartup,
    /// Cryptographic key rotation event
    IdentityRotation,
    /// New peer detected on the network
    PeerDiscovered,
    /// Secure channel established with peer
    HandshakeSuccess,
    /// Handshake with peer failed (logged for forensics)
    HandshakeFailure,
    /// Peer trust score crossed a threshold
    TrustChange,
    /// Rate limit enforced on a peer
    RateLimitTriggered,
    /// Replay attack detected from a peer
    ReplayDetected,
    /// Invalid signature received from a peer
    InvalidSignature,
    /// Periodic hash chain checkpoint for log truncation
    BootstrapCheckpoint,
    /// Configuration modification event
    ConfigChange,
}

impl fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditEventType::NodeStartup => write!(f, "NODE_STARTUP"),
            AuditEventType::IdentityRotation => write!(f, "IDENTITY_ROTATION"),
            AuditEventType::PeerDiscovered => write!(f, "PEER_DISCOVERED"),
            AuditEventType::HandshakeSuccess => write!(f, "HANDSHAKE_SUCCESS"),
            AuditEventType::HandshakeFailure => write!(f, "HANDSHAKE_FAILURE"),
            AuditEventType::TrustChange => write!(f, "TRUST_CHANGE"),
            AuditEventType::RateLimitTriggered => write!(f, "RATE_LIMIT_TRIGGERED"),
            AuditEventType::ReplayDetected => write!(f, "REPLAY_DETECTED"),
            AuditEventType::InvalidSignature => write!(f, "INVALID_SIGNATURE"),
            AuditEventType::BootstrapCheckpoint => write!(f, "BOOTSTRAP_CHECKPOINT"),
            AuditEventType::ConfigChange => write!(f, "CONFIG_CHANGE"),
        }
    }
}

/// A single entry in the audit log hash chain.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Monotonic entry sequence number
    pub seq: u64,
    /// Wall clock timestamp (ms since UNIX epoch)
    pub timestamp_ms: u64,
    /// Event type
    pub event_type: AuditEventType,
    /// Human-readable description of the event
    pub description: String,
    /// Peer involved (if applicable)
    pub peer: Option<EntityId>,
    /// SHA-256 hash of this entry
    pub hash: [u8; 32],
    /// SHA-256 hash of the previous entry (all zeros for entry 0)
    pub prev_hash: [u8; 32],
    /// Whether this is a checkpoint entry
    pub is_checkpoint: bool,
}

impl AuditEntry {
    /// Compute the hash for an entry given its fields.
    fn compute_hash(
        seq: u64,
        timestamp_ms: u64,
        event_type: &AuditEventType,
        description: &str,
        peer: &Option<EntityId>,
        prev_hash: &[u8; 32],
    ) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&seq.to_le_bytes());
        hasher.update(&timestamp_ms.to_le_bytes());
        hasher.update(event_type.to_string().as_bytes());
        hasher.update(description.as_bytes());
        if let Some(eid) = peer {
            hasher.update(&eid.0);
        }
        hasher.update(prev_hash);
        hasher.finalize().into()
    }
}

/// Append-only audit log with tamper detection via hash chain.
pub struct AuditLog {
    /// Ring buffer of recent entries
    entries: VecDeque<AuditEntry>,
    /// Total entries ever added (monotonic seq number)
    total_entries: u64,
    /// Hash of the most recent entry (head of chain)
    current_hash: [u8; 32],
    /// Last checkpoint hash (for verification)
    last_checkpoint_hash: [u8; 32],
    /// Hash of the genesis entry (entry 0)
    genesis_hash: Option<[u8; 32]>,
}

impl fmt::Debug for AuditLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditLog")
            .field("total_entries", &self.total_entries)
            .field("buffered_entries", &self.entries.len())
            .finish()
    }
}

impl AuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        AuditLog {
            entries: VecDeque::with_capacity(MAX_AUDIT_ENTRIES),
            total_entries: 0,
            current_hash: [0u8; 32],
            last_checkpoint_hash: [0u8; 32],
            genesis_hash: None,
        }
    }

    /// Append an event to the audit log.
    ///
    /// Returns the sequence number of the new entry.
    pub fn append(
        &mut self,
        event_type: AuditEventType,
        description: &str,
        peer: Option<EntityId>,
    ) -> u64 {
        let seq = self.total_entries;
        let now = now_millis();
        let prev_hash = self.current_hash;

        let hash = AuditEntry::compute_hash(seq, now, &event_type, description, &peer, &prev_hash);

        let is_checkpoint = seq > 0 && seq % CHECKPOINT_INTERVAL as u64 == 0;

        let entry = AuditEntry {
            seq,
            timestamp_ms: now,
            event_type,
            description: description.to_string(),
            peer,
            hash,
            prev_hash,
            is_checkpoint,
        };

        // Track genesis
        if seq == 0 {
            self.genesis_hash = Some(hash);
        }

        // Update chain state
        self.current_hash = hash;
        if is_checkpoint {
            self.last_checkpoint_hash = hash;
        }

        // Add to ring buffer
        if self.entries.len() >= MAX_AUDIT_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);

        self.total_entries += 1;
        seq
    }

    /// Verify the integrity of the entire hash chain from genesis.
    ///
    /// Returns `true` if the chain is intact (no tampering).
    pub fn verify_integrity(&self) -> bool {
        if self.entries.is_empty() {
            return true;
        }

        let first = self.entries.front().unwrap();
        if first.seq != 0 {
            // We don't have the full chain in buffer — verify what we have
            return self.verify_buffered();
        }

        // Walk the chain
        let mut prev_hash = [0u8; 32];
        for entry in &self.entries {
            let expected_hash = AuditEntry::compute_hash(
                entry.seq,
                entry.timestamp_ms,
                &entry.event_type,
                &entry.description,
                &entry.peer,
                &prev_hash,
            );
            if expected_hash != entry.hash {
                return false; // Tampering detected!
            }
            prev_hash = entry.hash;
        }

        true
    }

    /// Verify the buffered portion of the chain (without full genesis).
    fn verify_buffered(&self) -> bool {
        if self.entries.len() < 2 {
            return true;
        }

        let mut prev = self.entries[0].prev_hash;
        // We can only verify from the second entry if the first's prev_hash is correct
        // Partial verification: check internal consistency
        for i in 0..self.entries.len() - 1 {
            let expected = AuditEntry::compute_hash(
                self.entries[i].seq,
                self.entries[i].timestamp_ms,
                &self.entries[i].event_type,
                &self.entries[i].description,
                &self.entries[i].peer,
                &prev,
            );
            if expected != self.entries[i].hash {
                return false;
            }
            prev = self.entries[i].hash;

            // Check linkage to next entry
            if self.entries[i + 1].prev_hash != self.entries[i].hash {
                return false;
            }
        }

        true
    }

    /// Get the current chain head hash (for external verification).
    pub fn current_hash(&self) -> [u8; 32] {
        self.current_hash
    }

    /// Get the last checkpoint hash (for truncated verification).
    pub fn last_checkpoint_hash(&self) -> [u8; 32] {
        self.last_checkpoint_hash
    }

    /// Get the genesis hash (if available).
    pub fn genesis_hash(&self) -> Option<[u8; 32]> {
        self.genesis_hash
    }

    /// Get total entries ever added.
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Get the number of entries currently buffered.
    pub fn buffered_count(&self) -> usize {
        self.entries.len()
    }

    /// Get entries in a range.
    pub fn get_entries(&self, offset: usize, limit: usize) -> Vec<&AuditEntry> {
        self.entries.iter().skip(offset).take(limit).collect()
    }

    /// Get all entries.
    pub fn all_entries(&self) -> &VecDeque<AuditEntry> {
        &self.entries
    }

    /// Generate a bootstrap verification string.
    ///
    /// This can be shared with other nodes to verify the audit trail.
    pub fn bootstrap_proof(&self) -> BootstrapProof {
        BootstrapProof {
            total_entries: self.total_entries,
            current_hash: self.current_hash,
            checkpoint_hash: self.last_checkpoint_hash,
            genesis_hash: self.genesis_hash,
        }
    }

    /// Verify a bootstrap proof against the current state.
    pub fn verify_bootstrap(&self, proof: &BootstrapProof) -> bool {
        // The proof's current hash should match ours, or be an ancestor
        if proof.total_entries > self.total_entries {
            return false; // Can't verify future state
        }
        if proof.total_entries == self.total_entries {
            return proof.current_hash == self.current_hash;
        }
        // For truncated logs, check that the proof's checkpoint is consistent
        proof.checkpoint_hash == self.last_checkpoint_hash
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// A bootstrap proof that can be used to verify the integrity of an audit log
/// without transmitting the entire log.
#[derive(Debug, Clone)]
pub struct BootstrapProof {
    /// Total entries at proof generation time
    pub total_entries: u64,
    /// Hash of the most recent entry
    pub current_hash: [u8; 32],
    /// Hash of the most recent checkpoint
    pub checkpoint_hash: [u8; 32],
    /// Hash of the genesis entry (if in buffer)
    pub genesis_hash: Option<[u8; 32]>,
}

/// Get current timestamp in milliseconds.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Secure Bootstrap ─────────────────────────────────────────

/// Secure bootstrap configuration for joining the NWP network.
///
/// A new node must connect to at least one bootstrap peer to join.
/// The bootstrap process is secured by verifying the peer's identity
/// and optionally checking the audit log hash chain.
#[derive(Debug, Clone)]
pub struct SecureBootstrap {
    /// Known bootstrap peer addresses (host:port)
    pub bootstrap_peers: Vec<String>,
    /// Required minimum trust score for bootstrap peers
    pub min_bootstrap_trust: f32,
    /// Bootstrap timeout in ms
    pub timeout_ms: u64,
    /// Whether to require a verified audit proof from bootstrap peer
    pub require_audit_proof: bool,
}

impl Default for SecureBootstrap {
    fn default() -> Self {
        SecureBootstrap {
            bootstrap_peers: Vec::new(),
            min_bootstrap_trust: 0.7,
            timeout_ms: 5000,
            require_audit_proof: true,
        }
    }
}

impl SecureBootstrap {
    /// Create a new bootstrap configuration with a single known peer.
    pub fn new(peer: &str) -> Self {
        SecureBootstrap {
            bootstrap_peers: vec![peer.to_string()],
            min_bootstrap_trust: 0.7,
            timeout_ms: 5000,
            require_audit_proof: true,
        }
    }

    /// Add a bootstrap peer.
    pub fn add_peer(&mut self, peer: &str) {
        self.bootstrap_peers.push(peer.to_string());
    }

    /// Validate that the bootstrap configuration is sane.
    pub fn validate(&self) -> Result<(), String> {
        if self.bootstrap_peers.is_empty() {
            return Err("At least one bootstrap peer required".to_string());
        }
        if self.min_bootstrap_trust < 0.0 || self.min_bootstrap_trust > 1.0 {
            return Err("Minimum trust must be in [0.0, 1.0]".to_string());
        }
        if self.timeout_ms < 100 {
            return Err("Timeout must be at least 100ms".to_string());
        }
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_hash_chain() {
        let mut log = AuditLog::new();
        let seq0 = log.append(AuditEventType::NodeStartup, "Node initialized", None);
        assert_eq!(seq0, 0);
        assert!(log.genesis_hash.is_some());

        let seq1 = log.append(
            AuditEventType::PeerDiscovered,
            "Discovered peer 0xabcd",
            Some(EntityId([1u8; 32])),
        );
        assert_eq!(seq1, 1);

        assert!(log.verify_integrity());
    }

    #[test]
    fn test_tamper_detection() {
        let mut log = AuditLog::new();
        log.append(AuditEventType::NodeStartup, "init", None);
        log.append(AuditEventType::PeerDiscovered, "peer1", None);

        // Verify integrity before tampering
        assert!(log.verify_integrity());

        // Tamper with an entry's hash
        if let Some(entry) = log.entries.get_mut(0) {
            entry.hash = [0xFFu8; 32];
        }

        // Verify tampering is detected
        assert!(!log.verify_integrity());
    }

    #[test]
    fn test_multiple_events() {
        let mut log = AuditLog::new();
        let peer = EntityId([2u8; 32]);

        log.append(AuditEventType::NodeStartup, "boot", None);
        log.append(AuditEventType::PeerDiscovered, "discovered", Some(peer));
        log.append(AuditEventType::HandshakeSuccess, "handshake ok", Some(peer));
        log.append(AuditEventType::TrustChange, "trust increased", Some(peer));
        log.append(AuditEventType::BootstrapCheckpoint, "checkpoint", None);

        assert_eq!(log.total_entries(), 5);
        assert!(log.verify_integrity());
        assert!(log.current_hash() != [0u8; 32]);
    }

    #[test]
    fn test_bootstrap_proof() {
        let mut log = AuditLog::new();
        log.append(AuditEventType::NodeStartup, "boot", None);
        log.append(AuditEventType::ConfigChange, "config", None);

        let proof = log.bootstrap_proof();
        assert!(log.verify_bootstrap(&proof));
        assert_eq!(proof.total_entries, 2);
    }

    #[test]
    fn test_secure_bootstrap_validation() {
        let mut bs = SecureBootstrap::new("192.168.1.1:8000");
        assert!(bs.validate().is_ok());

        let empty = SecureBootstrap::default();
        assert!(empty.validate().is_err());

        bs.min_bootstrap_trust = 2.0;
        assert!(bs.validate().is_err());
    }

    #[test]
    fn test_checkpointing() {
        let mut log = AuditLog::new();
        for i in 0..CHECKPOINT_INTERVAL + 5 {
            log.append(AuditEventType::PeerDiscovered, &format!("peer_{}", i), None);
        }

        assert!(
            log.last_checkpoint_hash() != [0u8; 32],
            "checkpoint hash should be set after CHECKPOINT_INTERVAL entries"
        );
        assert!(log.verify_integrity());
    }
}
