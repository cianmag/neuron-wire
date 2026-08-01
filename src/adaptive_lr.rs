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
/// every synapse has its own accumulator(s). State is mutated through a
/// `RefCell` (interior mutability) so `update()` can be called with `&self`.
#[derive(Debug, Clone)]
pub struct AdaptiveLROptimiser {
    /// The selected strategy.
    pub config: AdaptiveLR,
    /// Per-synapse state (interior mutation).
    state: std::cell::RefCell<std::collections::HashMap<(EntityId, EntityId), PerSynapseState>>,
}

impl AdaptiveLROptimiser {
    /// Create a new optimiser with the given strategy.
    pub fn new(config: AdaptiveLR) -> Self {
        AdaptiveLROptimiser {
            config,
            state: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    /// Compute the effective learning rate for one synapse.
    ///
    /// Returns `η_eff` such that the weight update is `w += η_eff * grad`.
    ///
    /// State is accumulated per-synapse (AdaGrad / RMSProp / Adam) via
    /// interior mutability (`RefCell`), so this can be called with `&self`
    /// from a concurrent tick.
    #[allow(unused_variables)]
    pub fn update(&self, synapse_id: (EntityId, EntityId), grad: f32, w: f32) -> f32 {
        match &self.config {
            AdaptiveLR::Fixed(eta) => *eta,
            AdaptiveLR::AdaGrad { eta, eps } => {
                let mut s = self.get_state_mut(synapse_id);
                s.g2 += grad * grad;
                *eta / (s.g2.sqrt() + eps)
            }
            AdaptiveLR::RMSProp { eta, rho, eps } => {
                let mut s = self.get_state_mut(synapse_id);
                s.g2 = rho * s.g2 + (1.0 - rho) * grad * grad;
                *eta / (s.g2.sqrt() + eps)
            }
            AdaptiveLR::Adam {
                eta,
                beta1,
                beta2,
                eps,
            } => {
                let mut s = self.get_state_mut(synapse_id);
                s.t = s.t.saturating_add(1); // Adam steps start at t=1
                s.m = beta1 * s.m + (1.0 - beta1) * grad;
                s.g2 = beta2 * s.g2 + (1.0 - beta2) * grad * grad;
                // Bias correction — t >= 1 guarantees the denominator is
                // non-zero (beta < 1 ⇒ 1 - beta^t > 0).
                let denom1 = 1.0 - beta1.powi(s.t as i32);
                let denom2 = 1.0 - beta2.powi(s.t as i32);
                let m_hat = if denom1 > 0.0 { s.m / denom1 } else { s.m };
                let v_hat = if denom2 > 0.0 { s.g2 / denom2 } else { s.g2 };
                let denom = v_hat.sqrt() + eps;
                if denom > 0.0 {
                    eta / denom * m_hat.abs().max(1e-8)
                } else {
                    *eta
                }
            }
        }
    }

    /// Get or create the state entry for a synapse (interior mutation).
    fn get_state_mut(&self, id: (EntityId, EntityId)) -> std::cell::RefMut<'_, PerSynapseState> {
        std::cell::RefMut::map(self.state.borrow_mut(), |map| map.entry(id).or_default())
    }

    /// Get a snapshot of the state entry for a synapse (read-only).
    #[cfg(test)]
    fn get_state(&self, id: (EntityId, EntityId)) -> PerSynapseState {
        self.state.borrow().get(&id).cloned().unwrap_or_default()
    }

    /// Reset the state for a specific synapse.
    pub fn reset(&mut self, id: &(EntityId, EntityId)) {
        self.state.borrow_mut().remove(id);
    }

    /// Reset all per-synapse state.
    pub fn clear(&mut self) {
        self.state.borrow_mut().clear();
    }

    /// Number of tracked synapses.
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }

    /// Returns `true` if no synapses are tracked.
    pub fn is_empty(&self) -> bool {
        self.state.borrow().is_empty()
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
        // Default strategy is adaptive (Adam), not fixed-rate.
        assert!(matches!(opt.config, AdaptiveLR::Adam { .. }));
    }

    #[test]
    fn adam_first_step_is_finite() {
        // Regression: the original Adam branch divided by (1 - beta^t) with
        // t=0 → 0/0 = NaN on the very first update, poisoning every synapse
        // weight in any brain-attached engine. Steps start at t=1 and the
        // state accumulates, so η_eff must be finite on every call.
        let opt = AdaptiveLROptimiser::default();
        let sid = (eid(1), eid(2));
        for step in 0..5 {
            let eta = opt.update(sid, 0.5, 0.1);
            assert!(
                eta.is_finite(),
                "η_eff must be finite at step {} (was {})",
                step,
                eta
            );
            assert!(eta > 0.0, "η_eff must be positive at step {}", step);
        }
        // State accumulated: after 5 steps the per-synapse counter advanced.
        let s = opt.get_state(sid);
        assert_eq!(s.t, 5, "Adam step counter must advance");
        assert!(s.g2 > 0.0, "second-moment accumulator must accumulate");
    }

    #[test]
    fn adam_zero_gradient_stays_finite() {
        // Zero gradient must not produce NaN either (m=0, v=0).
        let opt = AdaptiveLROptimiser::default();
        let eta = opt.update((eid(3), eid(4)), 0.0, 0.1);
        assert!(eta.is_finite(), "zero-gradient η_eff must be finite");
    }
}
