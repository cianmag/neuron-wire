//! Message type identifiers and body layout constants.

/// NWP Message types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Ping = 0,
    Pong = 1,
    Command = 2,
    Spike = 3,
    Readiness = 4,
    Data = 5,
    Consensus = 6,
}

impl MsgType {
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
            MsgType::Command => 36,  // 7 scalar fields + 2 offsets (name + metadata)
            MsgType::Spike => 32,    // 5 scalar fields + 1 offset (payload)
            MsgType::Readiness => 20, // 3 scalars + 3 padding
            MsgType::Data => 24,     // DataHeader: sender_id + data_hash + content_type + compression + original_len + payload_len
            MsgType::Consensus => 24, // proposal_id + voter_id + confidence_raw + flags
        }
    }
}

// ─── Field offsets for each message type ────────────────────────

/// COMMAND body layout (36 bytes fixed + optional data)
pub mod cmd {
    pub const COMMAND_ID: usize = 0;       // u32
    pub const PREDICTION_CODE: usize = 4;  // u32
    pub const CONFIDENCE: usize = 8;       // u32 (raw, 0-1_000_000)
    pub const CONTEXT_HASH: usize = 12;    // u32
    pub const DEADLINE_US: usize = 16;     // u32
    pub const SOURCE_ID: usize = 20;       // u64
    pub const TARGET_MASK: usize = 28;     // u32
    pub const NAME_OFFSET: usize = 32;     // u32 (relative offset to string in data region)
    // Optional data follows at fixed_size (36)
    pub const SIZE: usize = 36;
}

/// SPIKE body layout (32 bytes fixed + optional data)
pub mod spike {
    pub const SOURCE_ID: usize = 0;       // u64
    pub const TARGET_ID: usize = 8;       // u64
    pub const SPIKE_TYPE: usize = 16;     // u32
    pub const STRENGTH: usize = 20;       // u32 (raw)
    pub const PAYLOAD_HASH: usize = 24;   // u32
    pub const PAYLOAD_OFFSET: usize = 28; // u32 (relative offset)
    pub const SIZE: usize = 32;
}

/// READINESS body layout (20 bytes fixed)
pub mod readiness {
    pub const NEURON_ID: usize = 0;       // u64
    pub const COMMAND_ID: usize = 8;      // u32
    pub const LATENCY_US: usize = 12;     // u32
    pub const CACHE_HIT: usize = 16;      // u32 (0 or 1)
    // padding to 20
    pub const SIZE: usize = 20;
}

/// DATA body layout (24 bytes fixed header + payload bytes)
pub mod data {
    pub const SENDER_ID: usize = 0;       // u64
    pub const DATA_HASH: usize = 8;       // u32
    pub const CONTENT_TYPE: usize = 12;   // u16
    pub const COMPRESSION: usize = 14;    // u16
    pub const ORIGINAL_LEN: usize = 16;   // u32
    pub const PAYLOAD_LEN: usize = 20;    // u32
    pub const HEADER_SIZE: usize = 24;    // fixed header size
}

/// CONSENSUS body layout (24 bytes fixed)
pub mod consensus {
    pub const PROPOSAL_ID: usize = 0;     // u64
    pub const VOTER_ID: usize = 8;        // u64
    pub const CONFIDENCE: usize = 16;     // u32 (raw)
    pub const FLAGS: usize = 20;          // u32
    pub const SIZE: usize = 24;
}

/// Confidence encoding (u32: 0 = 0.0, 1_000_000 = 1.0)
pub const CONF_SCALE: f64 = 1_000_000.0;

#[inline]
pub fn conf_to_raw(c: f64) -> u32 {
    (c.clamp(0.0, 1.0) * CONF_SCALE) as u32
}

#[inline]
pub fn conf_from_raw(r: u32) -> f64 {
    (r as f64) / CONF_SCALE
}

/// Spike type constants
pub mod spike_type {
    pub const ACTIVATION: u32 = 0;
    pub const INHIBITION: u32 = 1;
    pub const MODULATION: u32 = 2;
}

/// Content type constants for DATA messages
pub mod content_type {
    pub const ACTIVATIONS: u16 = 0;
    pub const EMBEDDINGS: u16 = 1;
    pub const TEXT: u16 = 2;
    pub const WEIGHTS: u16 = 3;
    pub const GRADIENTS: u16 = 4;
    pub const ROUTING: u16 = 5;
}

/// Compression type constants
pub mod compression {
    pub const NONE: u16 = 0;
    pub const ZSTD: u16 = 1;
    pub const LZ4: u16 = 2;
}

/// Flag bits for MessageHeader.flags
pub mod flags {
    pub const COMPRESSED: u16 = 0x0001;
    pub const ENCRYPTED: u16 = 0x0002;
    pub const BROADCAST: u16 = 0x0004;
    pub const URGENT: u16 = 0x0008;
    pub const RESPONSE_EXPECTED: u16 = 0x0010;
}

/// Brain region masks (for target_mask in Command)
pub mod regions {
    pub const SENSORY: u32 = 1 << 0;
    pub const LANGUAGE: u32 = 1 << 1;
    pub const REASONING: u32 = 1 << 2;
    pub const MEMORY: u32 = 1 << 3;
    pub const MOTOR: u32 = 1 << 4;
    pub const VISUAL: u32 = 1 << 5;
    pub const AUDIO: u32 = 1 << 6;
    pub const EXECUTIVE: u32 = 1 << 7;
    pub const ALL: u32 = 0xFFFF_FFFF;
}

/// Dictionary-encoded prediction types
pub mod prediction {
    pub const CODE: u32 = 1;
    pub const MATH: u32 = 2;
    pub const TEXT: u32 = 3;
    pub const VISION: u32 = 4;
    pub const AUDIO: u32 = 5;
    pub const ACTION: u32 = 6;
    pub const MEMORY: u32 = 7;
    pub const SCIENCE: u32 = 8;
    pub const LOGIC: u32 = 9;
    pub const TRANSLATE: u32 = 10;
    pub const SUMMARIZE: u32 = 11;
    pub const QA: u32 = 12;
    pub const CREATIVE: u32 = 13;
    pub const DEBUG: u32 = 14;
    pub const DATA: u32 = 15;
    pub const PLAN: u32 = 16;
    pub const TOOL: u32 = 17;
    pub const MULTIMODAL: u32 = 18;
    pub const LEARNING: u32 = 19;
    pub const SOCIAL: u32 = 20;

    pub fn name(code: u32) -> &'static str {
        match code {
            1 => "code", 2 => "math", 3 => "text", 4 => "vision",
            5 => "audio", 6 => "action", 7 => "memory", 8 => "science",
            9 => "logic", 10 => "translate", 11 => "summarize", 12 => "qa",
            13 => "creative", 14 => "debug", 15 => "data", 16 => "plan",
            17 => "tool", 18 => "multimodal", 19 => "learning", 20 => "social",
            _ => "unknown",
        }
    }
}
