//! Security Subsystem — unified entry point for the NWP security layer.
//!
//! This module re-exports the four security subsystems and provides
//! integration helpers that compose them.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────────┐     ┌────────────┐     ┌──────────┐
//! │  identity   │────→│  secure_channel │────→│   trust    │────→│  audit   │
//! │ (keypairs,  │     │  (encryption,   │     │ (scoring,  │     │ (hash    │
//! │  signing,   │     │   handshake,    │     │  sybil,    │     │  chain,  │
//! │  EntityId)  │     │   replay-prot)  │     │  rate-lmt) │     │  proofs) │
//! └─────────────┘     └─────────────────┘     └────────────┘     └──────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use neuron_wire::security::{NodeIdentity, SecureChannel, TrustSystem, AuditLog};
//! ```

pub use crate::audit::{AuditEntry, AuditEventType, AuditLog, BootstrapProof, SecureBootstrap};
pub use crate::identity::{
    entity_id_from_public_key, verify_packet_signature, verify_signature, NodeIdentity,
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};
pub use crate::secure_channel::{now_millis, SecureChannel};
pub use crate::trust::{
    TrustEvent, TrustStats, TrustSystem, INITIAL_TRUST, MAX_TRACKED_PEERS, RATE_LIMIT_BURST,
    SYBIL_THRESHOLD, TRUSTED_THRESHOLD,
};
