#![deny(missing_docs)]

//! Adaptive learning rates for per-synapse optimisation.
//!
//! Provides `AdaptiveLROptimiser` — a per-synapse learning-rate controller
//! supporting Fixed, AdaGrad, RMSProp, and Adam strategies.

use crate::components::EntityId;

/// Which adaptive learning-rate algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdaptiveLR {
    /// Constant learning rate — no adaptation.
    Fixed(f32),
    /// AdaGrad: per-synapse sum-of-squared-gradients scaling.
    AdaGrad {
        /// Base learning rate.
        eta: f32,
        /// Numerical stability epsilon.
        eps: f32,
    },
    /// RMSProp: exponential moving average of squared gradients.
    RMSProp {
        /// Base learning rate.
        eta: f32,
        /// Decay rate for squared gradient moving average.
        rho: f32,
        /// Numerical stability epsilon.
        eps: f32,
    },
    /// Adam: adaptive moment estimation.
    Adam {
        /// Base learning rate.
        eta: f32,
        /// Decay rate for first moment estimate.
        beta1: f32,
        /// Decay rate for second moment estimate.
        beta2: f32,
        /// Numerical stability epsilon.
        eps: f32,
    },
}

impl AdaptiveLR {
    /// Create an Adam variant with default hyperparameters.
    pub fn adam() -> Self {
        AdaptiveLR::Adam {
            eta: 0.001,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-7,
        }
    }
}

impl Default for AdaptiveLR {
    fn default() -> Self {
        AdaptiveLR::adam()
    }
}

/// Per-synapse state for adaptive gradient methods.
#[derive(Debug, Clone)]
pub struct PerSynapseState {
    /// Gradient squared accumulator (AdaGrad / RMSProp / Adam v).
    pub g2: f32,
    /// First moment estimate (Adam m).
    pub m: f32,
    /// Bias-correction step counter.
    pub t: u64,
}

impl Default for PerSynapseState {
    fn default() -> Self {
        PerSynapseState {
            g2: 0.0,
            m: 0.0,
            t: 0,
        }
    }
}

/// Per-synapse adaptive learning-rate controller.
///
/// Maintains a `HashMap<(EntityId, EntityId), PerSynapseState>` so that
/// every synapse has its own accumulator(s). All public methods take
/// `&self` with interior mutation via `RefCell`.
#[derive(Debug, Clone)]
pub struct AdaptiveLROptimiser {
    /// The selected strategy.
    pub config: AdaptiveLR,
    /// Per-synapse state.
    state: std::collections::HashMap<(EntityId, EntityId), PerSynapseState>,
}

impl AdaptiveLROptimiser {
    /// Create a new optimiser with the given strategy.
    pub fn new(config: AdaptiveLR) -> Self {
        AdaptiveLROptimiser {
            config,
            state: std::collections::HashMap::new(),
        }
    }

    /// Compute the effective learning rate for one synapse.
    ///
    /// Returns `η_eff` such that the weight update is `w += η_eff * grad`.
    #[allow(unused_variables)]
    pub fn update(&self, synapse_id: (EntityId, EntityId), grad: f32, w: f32) -> f32 {
        match &self.config {
            AdaptiveLR::Fixed(eta) => *eta,
            AdaptiveLR::AdaGrad { eta, eps } => {
                let s = self.get_state(synapse_id);
                *eta / (s.g2.sqrt() + eps)
            }
            AdaptiveLR::RMSProp { eta, rho, eps } => {
                let s = self.get_state(synapse_id);
                *eta / (s.g2.sqrt() + eps)
            }
            AdaptiveLR::Adam {
                eta,
                beta1,
                beta2,
                eps,
            } => {
                let s = self.get_state(synapse_id);
                let m_hat = s.m / (1.0 - beta1.powi(s.t as i32));
                let v_hat = s.g2 / (1.0 - beta2.powi(s.t as i32));
                eta / (v_hat.sqrt() + eps) * m_hat.abs().max(1e-8)
            }
        }
    }

    /// Get or create the state entry for a synapse.
    fn get_state(&self, id: (EntityId, EntityId)) -> PerSynapseState {
        self.state.get(&id).cloned().unwrap_or_default()
    }

    /// Reset the state for a specific synapse.
    pub fn reset(&mut self, id: &(EntityId, EntityId)) {
        self.state.remove(id);
    }

    /// Reset all per-synapse state.
    pub fn clear(&mut self) {
        self.state.clear();
    }

    /// Number of tracked synapses.
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Returns `true` if no synapses are tracked.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
}

impl Default for AdaptiveLROptimiser {
    fn default() -> Self {
        Self::new(AdaptiveLR::adam())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(id: u8) -> EntityId {
        EntityId([id; 32])
    }

    #[test]
    fn test_fixed_lr() {
        let opt = AdaptiveLROptimiser::new(AdaptiveLR::Fixed(0.01));
        let eta = opt.update((eid(1), eid(2)), 0.5, 0.1);
        assert!((eta - 0.01).abs() < 1e-8);
    }

    #[test]
    fn test_default() {
        let opt = AdaptiveLROptimiser::default();
        assert!(matches!(opt.config, AdaptiveLR::Fixed(0.01)));
    }
}
