//! Spike message — activation signal between neurons.
//!
//! When a neuron fires, it sends a spike to its connected neurons.
//! The spike is tiny (28 bytes minimum) and carries no data — only
//! a hash. If the receiving neuron needs the payload, it fetches it
//! lazily via a DATA message.
//!
//! This is how the brain avoids moving data around: spikes are
//! just triggers. The real data is fetched on demand.

use crate::NeuronId;

/// Spike message body — 28 bytes fixed + optional payload.
///
/// Binary layout:
/// ```text
/// [0-7]   source_id: u64       = sender neuron ID
/// [8-15]  target_id: u64       = target neuron ID
/// [16-19] spike_type: u32      = type of spike
/// [20-23] strength_raw: u32    = activation strength (0-1000000)
/// [24-27] payload_hash: u32    = hash of payload (0 = no payload)
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SpikeBody {
    /// Sender neuron ID
    pub source_id: NeuronId,
    /// Target neuron ID
    pub target_id: NeuronId,
    /// Spike type (activation, inhibition, modulation)
    pub spike_type: u32,
    /// Activation strength (0-1000000)
    pub strength_raw: u32,
    /// CRC32 of the payload (0 = no payload attached)
    pub payload_hash: u32,
}

impl SpikeBody {
    /// Minimum size (with optional payload after)
    pub const MIN_SIZE: usize = 28;

    /// Spike type constants
    pub const ACTIVATION: u32 = 0;
    pub const INHIBITION: u32 = 1;
    pub const MODULATION: u32 = 2;
    pub const CONSOLIDATION: u32 = 3;

    /// Create a new spike
    pub fn new(
        source_id: NeuronId,
        target_id: NeuronId,
        spike_type: u32,
        strength: f64,
        payload_hash: u32,
    ) -> Self {
        SpikeBody {
            source_id,
            target_id,
            spike_type,
            strength_raw: crate::confidence_to_raw(strength),
            payload_hash,
        }
    }

    /// Get activation strength as f64
    #[inline]
    pub fn strength(&self) -> f64 {
        crate::confidence_from_raw(self.strength_raw)
    }

    /// Zero-copy: interpret bytes as a SpikeBody.
    /// # Safety
    /// The slice must be at least 28 bytes.
    #[inline]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &SpikeBody {
        assert!(bytes.len() >= Self::MIN_SIZE);
        &*(bytes.as_ptr() as *const SpikeBody)
    }
}
