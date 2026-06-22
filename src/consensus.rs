//! Consensus message — voting between neurons for distributed agreement.
//!
//! When multiple brain regions propose different answers, consensus
//! messages are exchanged until a majority emerges.

use crate::NeuronId;

/// Consensus message body — 24 bytes, fixed-size.
///
/// Binary layout:
/// ```text
/// [0-7]   proposal_id: u64    = unique proposal identifier
/// [8-15]  voter_id: u64       = voting neuron
/// [16-19] confidence_raw: u32 = voter's confidence in their vote
/// [20-23] flags: u32          = vote value + metadata
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ConsensusBody {
    /// Proposal ID (hash of the proposal content)
    pub proposal_id: u64,
    /// Neuron ID of the voter
    pub voter_id: NeuronId,
    /// Voter's confidence (0-1000000)
    pub confidence_raw: u32,
    /// Flags (bits 0-1: vote, rest: metadata)
    pub flags: u32,
}

impl ConsensusBody {
    /// Size in bytes
    pub const SIZE: usize = 24;

    /// Vote values (stored in bits 0-1 of flags)
    pub const VOTE_NO: u32 = 0;
    pub const VOTE_YES: u32 = 1;
    pub const VOTE_ABSTAIN: u32 = 2;

    /// Get the vote value
    #[inline]
    pub fn vote(&self) -> u32 {
        self.flags & 0x03
    }

    /// Get voter confidence as f64
    #[inline]
    pub fn confidence(&self) -> f64 {
        crate::confidence_from_raw(self.confidence_raw)
    }

    /// Zero-copy: interpret bytes as a ConsensusBody
    #[inline]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &ConsensusBody {
        assert!(bytes.len() >= Self::SIZE);
        &*(bytes.as_ptr() as *const ConsensusBody)
    }
}
