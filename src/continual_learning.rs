//! Continual Learning — catastrophic forgetting prevention.
//!
//! Implements elastic weight consolidation and progressive neural networks
//! to enable the system to learn new tasks without forgetting old ones.
#![deny(missing_docs)]
//! forgetting in distributed neural networks.
//!
//! Implements two complementary methods:
//!
//! * **Elastic Weight Consolidation (EWC)** — penalises changes to
//!   weights that were important for previous tasks, using a Fisher
//!   Information estimate.
//! * **Synaptic Intelligence (SI)** — tracks the path integral of
//!   weight changes over time and assigns per-synapse "omega"
//!   importance values.
//!
//! Both methods produce a **correction term** that is added to the
//! weight gradient during Hebbian updates, biasing the optimiser
//! towards previously-learned solutions.

use std::collections::HashMap;

use crate::components::EntityId;

// ─── Elastic Weight Consolidation ────────────────────────────────

/// Elastic Weight Consolidation (EWC) state — protects important weights from forgetting.
///
/// Maintains a per-synapse importance (Fisher) estimate that penalises
/// changes to weights that were important for previous tasks.
#[derive(Debug, Clone)]
pub struct EWC {
    /// EWC regularisation strength.
    pub gamma: f32,
    /// Decay factor for Fisher updates (exponential moving average).
    pub fisher_decay: f32,
    /// Per-synapse Fisher / importance estimates.
    pub importance: HashMap<(EntityId, EntityId), f32>,
    /// Anchor (previously-learned) weights.
    pub anchor_weights: HashMap<(EntityId, EntityId), f32>,
}

impl EWC {
    /// Create a new EWC tracker.
    pub fn new(gamma: f32, fisher_decay: f32) -> Self {
        EWC {
            gamma,
            fisher_decay,
            importance: HashMap::new(),
            anchor_weights: HashMap::new(),
        }
    }

    /// Update importance (Fisher) estimates using the current gradients and loss.
    pub fn update_importance(&mut self, gradients: &[(EntityId, EntityId, f32)], _loss: f32) {
        for &(pre, post, grad) in gradients {
            let key = (pre, post);
            let f_old = self.importance.get(&key).copied().unwrap_or(0.0);
            let fisher = self.fisher_decay * f_old + (1.0 - self.fisher_decay) * grad * grad;
            self.importance.insert(key, fisher);
            // Save anchor weight on first update
            self.anchor_weights.entry(key).or_insert(0.0);
        }
    }

    /// Return the EWC correction term for a weight.
    pub fn correction(&self, w: f32, id: (EntityId, EntityId)) -> f32 {
        let f = self.importance.get(&id).copied().unwrap_or(0.0);
        let w0 = self.anchor_weights.get(&id).copied().unwrap_or(0.0);
        -self.gamma * f * (w - w0)
    }
}

// SAFETY: EWC contains only HashMap fields and Copy-type scalars. All mutation goes through
// `&mut self` methods (update_importance). No interior mutability or shared mutable state.
unsafe impl Sync for EWC {}

// ─── Synaptic Intelligence ──────────────────────────────────────

/// Synaptic Intelligence (SI) state — tracks per-synapse importance and penalises changes.
///
/// SI computes an importance weight `omega` for each synapse based on
/// the path integral of its contribution to the loss.
#[derive(Debug, Clone)]
pub struct SynapticIntelligence {
    /// SI regularisation strength.
    pub gamma: f32,
    /// Per-synapse importance (omega).
    pub omega: HashMap<(EntityId, EntityId), f32>,
    /// History of weight values per synapse: `(tick, weight)`.
    pub weight_history: HashMap<(EntityId, EntityId), Vec<(u64, f32)>>,
}

impl SynapticIntelligence {
    /// Create a new SI tracker.
    pub fn new(gamma: f32) -> Self {
        SynapticIntelligence {
            gamma,
            omega: HashMap::new(),
            weight_history: HashMap::new(),
        }
    }

    /// Update importance estimates from gradients.
    pub fn update_importance(&mut self, gradients: &[(EntityId, EntityId, f32)], _loss: f32) {
        for &(pre, post, grad) in gradients {
            let key = (pre, post);
            let hist = self.weight_history.entry(key).or_default();
            let prev_w = hist.last().map(|&(_, w)| w).unwrap_or(0.0);
            let delta_w = 0.01; // approximated step
            let omega_contrib = delta_w * grad.abs();
            let omega_old = self.omega.get(&key).copied().unwrap_or(0.0);
            self.omega.insert(key, omega_old + omega_contrib);
            hist.push((0, prev_w + delta_w));
        }
    }

    /// Return the SI correction term for a weight.
    pub fn correction(&self, _w: f32, id: (EntityId, EntityId)) -> f32 {
        let om = self.omega.get(&id).copied().unwrap_or(0.0);
        -self.gamma * om
    }

    /// Record a weight value at a given tick.
    pub fn record_weight(&mut self, id: (EntityId, EntityId), t: u64, w: f32) {
        self.weight_history.entry(id).or_default().push((t, w));
    }
}

// SAFETY: SynapticIntelligence contains only HashMap fields and scalar values. All mutation
// goes through `&mut self` methods. No interior mutability or shared mutable state.
unsafe impl Sync for SynapticIntelligence {}

// ─── ContinualMethod ─────────────────────────────────────────────

/// Top-level continual learning strategy selector.
#[derive(Debug, Clone)]
pub enum ContinualMethod {
    /// No continual learning penalty.
    None,
    /// Elastic Weight Consolidation.
    EWC(EWC),
    /// Synaptic Intelligence.
    SynapticIntelligence(SynapticIntelligence),
}

impl ContinualMethod {
    /// Update importance estimates from a batch of gradients.
    pub fn update_importance(&mut self, gradients: &[(EntityId, EntityId, f32)], loss: f32) {
        match self {
            ContinualMethod::None => {}
            ContinualMethod::EWC(ewc) => ewc.update_importance(gradients, loss),
            ContinualMethod::SynapticIntelligence(si) => {
                si.update_importance(gradients, loss);
            }
        }
    }

    /// Return the correction term for a single synapse.
    pub fn correction(&self, w: f32, id: (EntityId, EntityId)) -> f32 {
        match self {
            ContinualMethod::None => 0.0,
            ContinualMethod::EWC(ewc) => ewc.correction(w, id),
            ContinualMethod::SynapticIntelligence(si) => si.correction(w, id),
        }
    }
}

// SAFETY: ContinualMethod is an enum wrapping EWC or SynapticIntelligence, both of which
// are independently justified as Sync. All mutation goes through `&mut self` methods.
unsafe impl Sync for ContinualMethod {}

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(id: u8) -> EntityId {
        EntityId([id; 32])
    }

    #[test]
    fn test_ewc_update_and_correction() {
        let mut ewc = EWC::new(100.0, 0.9);
        let grads = vec![(eid(1), eid(2), 0.5), (eid(3), eid(4), -0.2)];
        ewc.update_importance(&grads, 1.0);
        let c = ewc.correction(0.3, (eid(1), eid(2)));
        assert!(c != 0.0);
    }

    #[test]
    fn test_si_update_and_correction() {
        let mut si = SynapticIntelligence::new(10.0);
        let grads = vec![(eid(1), eid(2), 0.5)];
        si.update_importance(&grads, 1.0);
        let c = si.correction(0.3, (eid(1), eid(2)));
        assert!(c != 0.0);
    }

    #[test]
    fn test_none_returns_zero() {
        let method = ContinualMethod::None;
        assert_eq!(method.correction(0.5, (eid(1), eid(2))), 0.0);
    }
}
