//! Neuron Wire Protocol v2 — Zero-Copy FlatBuffer over TCP
//!
//! ## Wire Format
//! ```text
//! [4 bytes]  frame_len: u32   (NWP message size, excludes this field)
//! [16 bytes] MessageHeader    (magic + version + type + flags + body_len + crc)
//! [N bytes]  Body             (FlatBuffer-encoded body)
//! ```
//!
//! ## Zero-Copy
//! The entire message (header + body) is read into a flat buffer.
//! All field access is through offset computation into the buffer.
//! No deserialization, no allocation, no parsing step.

#![deny(missing_docs)]

// ─── Core Protocol Modules ──────────────────────────────────────
pub mod adaptive_lr;
pub mod adversary;
pub mod apoptosis;
pub mod attention;
pub mod components;
pub mod continual_learning;
pub mod crc;
pub mod curiosity;
pub mod dht;
pub mod distillation;
pub mod distributed_replay;
pub mod dynamic_activation;
pub mod engine_loop;
pub mod flat;
pub mod forgetting_bench;
pub mod forward_pass;
pub mod gradient_compression;
pub mod header;
pub mod hebbian;
pub mod io;
pub mod memory_module;
pub mod meta_learning;
pub mod neurogenesis;
pub mod routing;
pub mod simulator;
pub mod sparse_tensor;
pub mod transport;
pub mod types;
pub mod uncertainty;
pub mod zerocopy;

pub mod ml;

// ── Security / Identity ───────────────────────────────────────
pub mod audit;
pub mod identity;
pub mod observability;
pub mod secure_channel;
pub mod security;
pub mod trust;

// Re-exports
pub use flat::*;
pub use header::*;
pub use io::*;
pub use ml::*;
pub use types::*;

/// Magic: "NWP\0"
pub const MAGIC: [u8; 4] = [0x4E, 0x57, 0x50, 0x00];
/// NWP protocol version (currently 2)
pub const VERSION: u8 = 2;
/// Maximum allowed message body size in bytes
pub const MAX_BODY_SIZE: u32 = 1_000_000_000;
/// Size of the message header in bytes (16)
pub const HEADER_SIZE: usize = 16;

/// 64-bit neuron ID — enough for 18 quintillion devices
pub type NeuronId = u64;
