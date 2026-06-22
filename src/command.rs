//! Command message — from command brain to regions.
//!
//! The command brain issues PREDICTIONS, not data. Each region receives
//! a command and pre-loads the relevant expert BEFORE input arrives.
//!
//! Fixed-size: 32 bytes body + 16 byte header = 48 bytes total.

use crate::NeuronId;

/// Command message body — 32 bytes, fixed-size.
///
/// Binary layout:
/// ```text
/// [0-3]   command_id: u32       = unique command identifier
/// [4-7]   prediction_code: u32  = which prediction (dictionary-encoded)
/// [8-11]  confidence_raw: u32   = 0-1000000 (0.0-1.0)
/// [12-15] context_hash: u32     = CRC32 of context for cache lookup
/// [16-19] deadline_us: u32      = deadline in microseconds from now
/// [20-27] source_id: u64        = command brain's neuron ID
/// [28-31] target_mask: u32      = bitmask of target regions
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommandBody {
    /// Unique command identifier
    pub command_id: u32,
    /// Dictionary-encoded prediction type
    pub prediction_code: u32,
    /// Prediction confidence (0 = 0.0, 1_000_000 = 1.0)
    pub confidence_raw: u32,
    /// CRC32 of context (for cache lookup / prediction matching)
    pub context_hash: u32,
    /// Deadline in microseconds from message creation
    pub deadline_us: u32,
    /// Neuron ID of the command brain that issued this
    pub source_id: NeuronId,
    /// Bitmask of target regions (0 = broadcast to all)
    pub target_mask: u32,
}

impl CommandBody {
    /// Size in bytes
    pub const SIZE: usize = 32;

    /// Create a new command
    pub fn new(
        command_id: u32,
        prediction_code: u32,
        confidence: f64,
        context_hash: u32,
        deadline_us: u32,
        source_id: NeuronId,
        target_mask: u32,
    ) -> Self {
        let confidence_raw = crate::confidence_to_raw(confidence);
        CommandBody {
            command_id,
            prediction_code,
            confidence_raw,
            context_hash,
            deadline_us,
            source_id,
            target_mask,
        }
    }

    /// Get confidence as f64
    #[inline]
    pub fn confidence(&self) -> f64 {
        crate::confidence_from_raw(self.confidence_raw)
    }

    /// Zero-copy: interpret bytes as a CommandBody.
    /// # Safety
    /// The slice must be at least 32 bytes.
    #[inline]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &CommandBody {
        assert!(bytes.len() >= Self::SIZE);
        &*(bytes.as_ptr() as *const CommandBody)
    }
}
