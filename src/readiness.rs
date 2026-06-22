//! Readiness message — region reports its preparation status to command brain.
//!
//! After receiving a COMMAND, each neuron in the region pre-loads its expert
//! and reports back with its readiness status. The command brain uses these
//! to decide when to EXECUTE.

use crate::NeuronId;

/// Readiness message body — 20 bytes, fixed-size.
///
/// Binary layout:
/// ```text
/// [0-7]   neuron_id: u64       = reporting neuron
/// [8-11]  command_id: u32      = which command this responds to
/// [12-15] latency_us: u32      = estimated processing latency (microseconds)
/// [16]    flags: u8            = bit 0: cache_hit, rest reserved
/// [17-19] padding: [u8; 3]    = alignment
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ReadinessBody {
    /// Neuron ID of the reporting device
    pub neuron_id: NeuronId,
    /// Command ID this readiness is for
    pub command_id: u32,
    /// Estimated processing latency in microseconds
    pub latency_us: u32,
    /// Flags (bit 0: cache_hit)
    pub flags: u8,
    /// Padding for alignment
    _padding: [u8; 3],
}

impl ReadinessBody {
    /// Size in bytes
    pub const SIZE: usize = 20;

    /// Bit flags for the flags field
    pub const CACHE_HIT: u8 = 0x01;

    /// Create a new readiness report
    pub fn new(neuron_id: NeuronId, command_id: u32, latency_us: u32, cache_hit: bool) -> Self {
        let flags = if cache_hit { Self::CACHE_HIT } else { 0 };
        ReadinessBody {
            neuron_id,
            command_id,
            latency_us,
            flags,
            _padding: [0; 3],
        }
    }

    /// Whether this was a cache hit (expert was already loaded)
    #[inline]
    pub fn is_cache_hit(&self) -> bool {
        self.flags & Self::CACHE_HIT != 0
    }

    /// Zero-copy: interpret bytes as a ReadinessBody.
    /// # Safety
    /// Slice must be at least 20 bytes.
    #[inline]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &ReadinessBody {
        assert!(bytes.len() >= Self::SIZE);
        &*(bytes.as_ptr() as *const ReadinessBody)
    }
}
