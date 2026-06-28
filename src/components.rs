//! ECS Data Components for the Planetary Brain.
//!
//! These are the fundamental data structures that represent individual
//! "neurons" in our Entity Component System. Unlike a dense tensor,
//! each neuron is an independent entity identified by a cryptographic
//! 256-bit ID — spawnable, pruneable, and routable over the NWP network.

#![allow(missing_docs)]
use std::fmt;

// ─── EntityId (256-bit Cryptographic) ──────────────────────────

/// A globally unique neuron identifier.
///
/// 256 bits = SHA-256 hash space. Collision probability at 10¹⁸ neurons
/// is ≈ 10⁻⁴¹ — effectively zero for planetary scale.
///
/// Generated on spawn. Never reused after apoptosis.
/// Routable over NWP as a key for DHT lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(pub [u8; 32]);

impl EntityId {
    pub fn new(bytes: [u8; 32]) -> Self { EntityId(bytes) }

    /// Abbreviated hex for display
    pub fn hex(&self) -> String {
        let mut s = String::with_capacity(10);
        for &b in &self.0[..4] { s.push_str(&format!("{:02x}", b)); }
        s.push('…');
        for &b in &self.0[28..] { s.push_str(&format!("{:02x}", b)); }
        s
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "🧠{}", self.hex())
    }
}

// ─── ActivationComponent ──────────────────────────────────────

/// The current activation state of a single neuron.
///
/// In the ECS, every neuron has exactly one ActivationComponent.
/// The `value` is the neuron's membrane potential after the most
/// recent ForwardPassSystem tick.
#[derive(Debug, Clone)]
pub struct ActivationComponent {
    /// Current membrane potential / activation value
    pub value: f32,
    /// Engine tick when this was last written
    pub last_updated_tick: u64,
}

// ─── SynapseComponent ─────────────────────────────────────────

/// The synaptic connections FROM this neuron TO upstream targets.
///
/// In this ECS, `target_entities[i]` has weight `weights[i]`.
/// This is the **causal upstream** direction: which nodes contributed
/// to this neuron's prediction error.
#[derive(Debug, Clone)]
pub struct SynapseComponent {
    /// Upstream entities this neuron listens to
    pub target_entities: Vec<EntityId>,
    /// Synaptic weights (same length as target_entities)
    pub weights: Vec<f32>,
    /// Accumulated gradient deltas for batch weight updates
    pub accumulated_gradients: Vec<f32>,
}

// ─── PredictionComponent ──────────────────────────────────────

/// Stores the most recent prediction and the ground truth for
/// computing the surprise signal (prediction error).
#[derive(Debug, Clone)]
pub struct PredictionComponent {
    /// The value the neuron predicted
    pub predicted: f32,
    /// The actual observed value
    pub actual: f32,
    /// Absolute error |predicted - actual|
    pub error: f32,
    /// Squared error (for Hebbian scaling)
    pub error_sq: f32,
}

impl PredictionComponent {
    pub fn new(predicted: f32, actual: f32) -> Self {
        let error = (predicted - actual).abs();
        PredictionComponent {
            predicted,
            actual,
            error,
            error_sq: error * error,
        }
    }

    /// The surprise signal: squared error normalized to [0, 1]
    /// 0 = perfect prediction, 1 = catastrophic failure
    pub fn surprise(&self) -> f32 {
        (self.error_sq / (self.error_sq + 1.0)).min(1.0)
    }
}

// ─── EntityMap (Convenience) ──────────────────────────────────

/// A type alias for the brain's main neuron storage.
/// Maps EntityId → ActivationComponent.
pub type ActivationMap = std::collections::HashMap<EntityId, ActivationComponent>;

/// Maps EntityId → SynapseComponent.
pub type SynapseMap = std::collections::HashMap<EntityId, SynapseComponent>;

/// Maps EntityId → PredictionComponent.
pub type PredictionMap = std::collections::HashMap<EntityId, PredictionComponent>;

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id_roundtrip() {
        let id = EntityId([42u8; 32]);
        assert_eq!(id.0[0], 42);
        assert_eq!(id.0[31], 42);
    }

    #[test]
    fn test_prediction_surprise() {
        let p = PredictionComponent::new(0.5, 0.5);
        assert!((p.surprise() - 0.0).abs() < 0.001);

        let p2 = PredictionComponent::new(0.0, 10.0);
        assert!((p2.surprise() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_synapse_size_consistency() {
        let targets = vec![EntityId([1u8; 32]), EntityId([2u8; 32])];
        let s = SynapseComponent {
            target_entities: targets.clone(),
            weights: vec![0.1, 0.2],
            accumulated_gradients: vec![0.0, 0.0],
        };
        assert_eq!(s.target_entities.len(), s.weights.len());
        assert_eq!(s.weights.len(), s.accumulated_gradients.len());
    }

    #[test]
    fn test_activation_value() {
        let mut a = ActivationComponent { value: 0.0, last_updated_tick: 0 };
        a.value = 0.85;
        a.last_updated_tick = 100;
        assert!((a.value - 0.85).abs() < 0.001);
        assert_eq!(a.last_updated_tick, 100);
    }
}
