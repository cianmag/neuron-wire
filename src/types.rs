//! Message type identifiers and body layout constants.

/// NWP message types — each variant maps to a specific wire-format body layout.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    /// Keep‑alive probe; no body payload.
    Ping = 0,
    /// Response to a Ping; no body payload.
    Pong = 1,
    /// Instruction for a neuron to perform a computation.
    Command = 2,
    /// Event signal (activation, inhibition, or modulation).
    Spike = 3,
    /// Resource-readiness report from a worker.
    Readiness = 4,
    /// Bulk payload transfer (activations, embeddings, text, weights, …).
    Data = 5,
    /// Consensus‑round vote (proposal, confidence, flags).
    Consensus = 6,
}

impl MsgType {
    /// Convert a raw `u8` discriminant into a `MsgType`, returning `None` for
    /// invalid or reserved values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(MsgType::Ping),
            1 => Some(MsgType::Pong),
            2 => Some(MsgType::Command),
            3 => Some(MsgType::Spike),
            4 => Some(MsgType::Readiness),
            5 => Some(MsgType::Data),
            6 => Some(MsgType::Consensus),
            _ => None,
        }
    }

    /// Fixed body size in bytes for this message type.
    /// Variable-length payloads (Data) have additional bytes after the fixed portion.
    pub fn fixed_size(&self) -> usize {
        match self {
            MsgType::Ping | MsgType::Pong => 0,
            MsgType::Command => 36, // 7 scalar fields + 2 offsets (name + metadata)
            MsgType::Spike => 32,   // 5 scalar fields + 1 offset (payload)
            MsgType::Readiness => 20, // 3 scalars + 3 padding
            MsgType::Data => 24, // DataHeader: sender_id + data_hash + content_type + compression + original_len + payload_len
            MsgType::Consensus => 24, // proposal_id + voter_id + confidence_raw + flags
        }
    }
}

// ─── Field offsets for each message type ────────────────────────

/// COMMAND body layout (36 bytes fixed + optional data).
pub mod cmd {
    /// Offset of the command‑ID field (`u32`).
    pub const COMMAND_ID: usize = 0;
    /// Offset of the prediction‑code field (`u32`).
    pub const PREDICTION_CODE: usize = 4;
    /// Offset of the raw‑confidence field (`u32`, 0–1_000_000).
    pub const CONFIDENCE: usize = 8;
    /// Offset of the context‑hash field (`u32`).
    pub const CONTEXT_HASH: usize = 12;
    /// Offset of the deadline timestamp (`u32`, microseconds).
    pub const DEADLINE_US: usize = 16;
    /// Offset of the source‑neuron ID (`u64`).
    pub const SOURCE_ID: usize = 20;
    /// Offset of the target‑region bitmask (`u32`).
    pub const TARGET_MASK: usize = 28;
    /// Offset of the name‑string offset (`u32`, relative to data region).
    pub const NAME_OFFSET: usize = 32;
    /// Total fixed‑body size for a Command message.
    pub const SIZE: usize = 36;
}

/// SPIKE body layout (32 bytes fixed + optional data).
pub mod spike {
    /// Offset of the source‑neuron ID (`u64`).
    pub const SOURCE_ID: usize = 0;
    /// Offset of the target‑neuron ID (`u64`).
    pub const TARGET_ID: usize = 8;
    /// Offset of the spike‑type field (`u32`).
    pub const SPIKE_TYPE: usize = 16;
    /// Offset of the signal‑strength field (`u32`, raw).
    pub const STRENGTH: usize = 20;
    /// Offset of the payload‑hash field (`u32`).
    pub const PAYLOAD_HASH: usize = 24;
    /// Offset of the payload‑data offset (`u32`, relative).
    pub const PAYLOAD_OFFSET: usize = 28;
    /// Total fixed‑body size for a Spike message.
    pub const SIZE: usize = 32;
}

/// READINESS body layout (20 bytes fixed).
pub mod readiness {
    /// Offset of the reporting‑neuron ID (`u64`).
    pub const NEURON_ID: usize = 0;
    /// Offset of the command‑ID being reported on (`u32`).
    pub const COMMAND_ID: usize = 8;
    /// Offset of the measured latency (`u32`, microseconds).
    pub const LATENCY_US: usize = 12;
    /// Offset of the cache‑hit indicator (`u32`, 0 or 1).
    pub const CACHE_HIT: usize = 16;
    /// Total fixed‑body size for a Readiness message.
    pub const SIZE: usize = 20;
}

/// DATA body layout (24 bytes fixed header + payload bytes).
pub mod data {
    /// Offset of the sender‑neuron ID (`u64`).
    pub const SENDER_ID: usize = 0;
    /// Offset of the data‑content hash (`u32`).
    pub const DATA_HASH: usize = 8;
    /// Offset of the content‑type discriminator (`u16`).
    pub const CONTENT_TYPE: usize = 12;
    /// Offset of the compression‑method field (`u16`).
    pub const COMPRESSION: usize = 14;
    /// Offset of the original (pre‑compression) payload length (`u32`).
    pub const ORIGINAL_LEN: usize = 16;
    /// Offset of the stored (possibly compressed) payload length (`u32`).
    pub const PAYLOAD_LEN: usize = 20;
    /// Total fixed‑header size for a Data message.
    pub const HEADER_SIZE: usize = 24;
}

/// CONSENSUS body layout (24 bytes fixed).
pub mod consensus {
    /// Offset of the proposal ID (`u64`).
    pub const PROPOSAL_ID: usize = 0;
    /// Offset of the voting‑neuron ID (`u64`).
    pub const VOTER_ID: usize = 8;
    /// Offset of the raw‑confidence value (`u32`, 0–1_000_000).
    pub const CONFIDENCE: usize = 16;
    /// Offset of the consensus‑flags bitfield (`u32`).
    pub const FLAGS: usize = 20;
    /// Total fixed‑body size for a Consensus message.
    pub const SIZE: usize = 24;
}

/// Confidence encoding (u32: 0 = 0.0, 1_000_000 = 1.0).
pub const CONF_SCALE: f64 = 1_000_000.0;

/// Convert a floating-point confidence value `[0.0, 1.0]` into the raw `u32`
/// wire representation (0–1_000_000). Values outside the range are clamped.
#[inline]
pub fn conf_to_raw(c: f64) -> u32 {
    (c.clamp(0.0, 1.0) * CONF_SCALE) as u32
}

/// Convert a raw `u32` confidence value (0–1_000_000) back into a floating‑point
/// value in `[0.0, 1.0]`.
#[inline]
pub fn conf_from_raw(r: u32) -> f64 {
    (r as f64) / CONF_SCALE
}

/// Spike type constants used in [`spike::SPIKE_TYPE`].
pub mod spike_type {
    /// Excitatory / activation signal.
    pub const ACTIVATION: u32 = 0;
    /// Inhibitory / suppression signal.
    pub const INHIBITION: u32 = 1;
    /// Modulatory / neuromodulator signal.
    pub const MODULATION: u32 = 2;
}

/// Content type constants for DATA messages (stored in `content_type` field).
pub mod content_type {
    /// Neuron activation vectors.
    pub const ACTIVATIONS: u16 = 0;
    /// Embedding vectors.
    pub const EMBEDDINGS: u16 = 1;
    /// Plain text payload.
    pub const TEXT: u16 = 2;
    /// Model weight deltas or snapshots.
    pub const WEIGHTS: u16 = 3;
    /// Gradient tensors for training.
    pub const GRADIENTS: u16 = 4;
    /// Routing / topology metadata.
    pub const ROUTING: u16 = 5;
}

/// Compression type constants (stored in `compression` field of Data messages).
pub mod compression {
    /// No compression applied.
    pub const NONE: u16 = 0;
    /// Zstandard compression.
    pub const ZSTD: u16 = 1;
    /// LZ4 compression.
    pub const LZ4: u16 = 2;
}

/// Flag bits for `MessageHeader.flags`.
pub mod flags {
    /// Payload is zstd‑ or lz4‑compressed.
    pub const COMPRESSED: u16 = 0x0001;
    /// Payload is encrypted.
    pub const ENCRYPTED: u16 = 0x0002;
    /// Message is a broadcast (no single target).
    pub const BROADCAST: u16 = 0x0004;
    /// High‑priority / time‑sensitive message.
    pub const URGENT: u16 = 0x0008;
    /// Sender expects a response for this message.
    pub const RESPONSE_EXPECTED: u16 = 0x0010;
}

/// Brain region masks (for `target_mask` in Command messages).
pub mod regions {
    /// Sensory processing region.
    pub const SENSORY: u32 = 1 << 0;
    /// Language processing region.
    pub const LANGUAGE: u32 = 1 << 1;
    /// Reasoning & logic region.
    pub const REASONING: u32 = 1 << 2;
    /// Memory storage & retrieval region.
    pub const MEMORY: u32 = 1 << 3;
    /// Motor control region.
    pub const MOTOR: u32 = 1 << 4;
    /// Visual processing region.
    pub const VISUAL: u32 = 1 << 5;
    /// Audio processing region.
    pub const AUDIO: u32 = 1 << 6;
    /// Executive / decision‑making region.
    pub const EXECUTIVE: u32 = 1 << 7;
    /// All regions selected (all bits set).
    pub const ALL: u32 = 0xFFFF_FFFF;
}

/// Dictionary‑encoded prediction types for COMMAND messages.
pub mod prediction {
    /// Software / code generation.
    pub const CODE: u32 = 1;
    /// Mathematical reasoning.
    pub const MATH: u32 = 2;
    /// Text generation.
    pub const TEXT: u32 = 3;
    /// Visual / image processing.
    pub const VISION: u32 = 4;
    /// Audio / speech processing.
    pub const AUDIO: u32 = 5;
    /// Physical or virtual action.
    pub const ACTION: u32 = 6;
    /// Memory recall / storage.
    pub const MEMORY: u32 = 7;
    /// Scientific reasoning.
    pub const SCIENCE: u32 = 8;
    /// Symbolic logic.
    pub const LOGIC: u32 = 9;
    /// Translation between languages.
    pub const TRANSLATE: u32 = 10;
    /// Summarization task.
    pub const SUMMARIZE: u32 = 11;
    /// Question‑answering task.
    pub const QA: u32 = 12;
    /// Creative / generative task.
    pub const CREATIVE: u32 = 13;
    /// Debugging / code‑fixing task.
    pub const DEBUG: u32 = 14;
    /// Data analysis / processing.
    pub const DATA: u32 = 15;
    /// Planning / scheduling task.
    pub const PLAN: u32 = 16;
    /// Tool‑use / function‑calling task.
    pub const TOOL: u32 = 17;
    /// Multimodal fusion task.
    pub const MULTIMODAL: u32 = 18;
    /// Learning / adaptation task.
    pub const LEARNING: u32 = 19;
    /// Social interaction / communication.
    pub const SOCIAL: u32 = 20;

    /// Return a human‑readable name for the given prediction `code`.
    ///
    /// Returns `"unknown"` for codes that do not match any defined prediction type.
    pub fn name(code: u32) -> &'static str {
        match code {
            1 => "code",
            2 => "math",
            3 => "text",
            4 => "vision",
            5 => "audio",
            6 => "action",
            7 => "memory",
            8 => "science",
            9 => "logic",
            10 => "translate",
            11 => "summarize",
            12 => "qa",
            13 => "creative",
            14 => "debug",
            15 => "data",
            16 => "plan",
            17 => "tool",
            18 => "multimodal",
            19 => "learning",
            20 => "social",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_type_from_u8_all() {
        assert_eq!(MsgType::from_u8(0), Some(MsgType::Ping));
        assert_eq!(MsgType::from_u8(1), Some(MsgType::Pong));
        assert_eq!(MsgType::from_u8(2), Some(MsgType::Command));
        assert_eq!(MsgType::from_u8(3), Some(MsgType::Spike));
        assert_eq!(MsgType::from_u8(4), Some(MsgType::Readiness));
        assert_eq!(MsgType::from_u8(5), Some(MsgType::Data));
        assert_eq!(MsgType::from_u8(6), Some(MsgType::Consensus));
        assert_eq!(MsgType::from_u8(7), None);
        assert_eq!(MsgType::from_u8(255), None);
    }

    #[test]
    fn test_msg_type_fixed_size() {
        assert_eq!(MsgType::Ping.fixed_size(), 0);
        assert_eq!(MsgType::Pong.fixed_size(), 0);
        assert_eq!(MsgType::Command.fixed_size(), 36);
        assert_eq!(MsgType::Spike.fixed_size(), 32);
        assert_eq!(MsgType::Readiness.fixed_size(), 20);
        assert_eq!(MsgType::Data.fixed_size(), 24);
        assert_eq!(MsgType::Consensus.fixed_size(), 24);
    }

    #[test]
    fn test_confidence_roundtrip() {
        let vals = [0.0, 0.5, 1.0, 0.001, 0.999];
        for &v in &vals {
            let raw = conf_to_raw(v);
            let back = conf_from_raw(raw);
            let diff = (v - back).abs();
            assert!(
                diff < 1e-6,
                "roundtrip failed for {}: raw={} back={}",
                v,
                raw,
                back
            );
        }
    }

    #[test]
    fn test_confidence_clamping() {
        assert_eq!(conf_to_raw(-0.5), 0);
        assert_eq!(conf_to_raw(1.5), 1_000_000);
    }

    #[test]
    fn test_prediction_name_known() {
        assert_eq!(prediction::name(1), "code");
        assert_eq!(prediction::name(2), "math");
        assert_eq!(prediction::name(20), "social");
    }

    #[test]
    fn test_prediction_name_unknown() {
        assert_eq!(prediction::name(0), "unknown");
        assert_eq!(prediction::name(99), "unknown");
    }

    #[test]
    fn test_flags_no_overlap() {
        let all = flags::COMPRESSED
            | flags::ENCRYPTED
            | flags::BROADCAST
            | flags::URGENT
            | flags::RESPONSE_EXPECTED;
        assert_eq!(all.count_ones(), 5);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)] // contract tests: constants must keep ordering
    fn test_regions_all() {
        assert_eq!(regions::ALL, 0xFFFF_FFFF);
        assert!(regions::ALL & regions::SENSORY != 0);
        assert!(regions::ALL & regions::EXECUTIVE != 0);
    }

    #[test]
    fn test_content_type_values() {
        assert_eq!(content_type::ACTIVATIONS, 0);
        assert_eq!(content_type::GRADIENTS, 4);
        assert_eq!(content_type::ROUTING, 5);
    }

    #[test]
    fn test_compression_values() {
        assert_eq!(compression::NONE, 0);
        assert_eq!(compression::ZSTD, 1);
        assert_eq!(compression::LZ4, 2);
    }

    #[test]
    fn test_spike_type_values() {
        assert_eq!(spike_type::ACTIVATION, 0);
        assert_eq!(spike_type::INHIBITION, 1);
        assert_eq!(spike_type::MODULATION, 2);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)] // contract tests: constants must keep ordering
    fn test_field_offset_ordering() {
        assert!(cmd::COMMAND_ID < cmd::PREDICTION_CODE);
        assert!(cmd::SOURCE_ID < cmd::TARGET_MASK);
        assert!(cmd::TARGET_MASK < cmd::NAME_OFFSET);
        assert!(spike::SOURCE_ID < spike::TARGET_ID);
        assert!(spike::STRENGTH < spike::PAYLOAD_HASH);
        assert!(data::SENDER_ID < data::DATA_HASH);
        assert!(data::PAYLOAD_LEN < data::HEADER_SIZE);
    }

    #[test]
    fn test_ms_type_debug_and_clone() {
        let t = MsgType::Data;
        assert_eq!(format!("{:?}", t), "Data");
        let cloned = t;
        assert_eq!(t, cloned);
    }
}
