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

pub mod flat;
pub mod header;
pub mod types;
pub mod io;
pub mod transport;
pub mod engine_loop;
pub mod dht;
pub mod apoptosis;
pub mod components;
pub mod neurogenesis;
pub mod hebbian;
pub mod forward_pass;
pub mod zerocopy;
pub mod simulator;
pub mod adversary;
pub mod crc;

pub use flat::*;
pub use header::*;
pub use types::*;
pub use io::*;

/// Magic: "NWP\0"
pub const MAGIC: [u8; 4] = [0x4E, 0x57, 0x50, 0x00];
pub const VERSION: u8 = 2;
pub const MAX_BODY_SIZE: u32 = 1_000_000_000;
pub const HEADER_SIZE: usize = 16;

/// 64-bit neuron ID — enough for 18 quintillion devices
pub type NeuronId = u64;
