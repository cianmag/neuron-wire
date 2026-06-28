//! Neurogenesis System — Reactive Neuron Spawning Driven by Surprise.
//!
//! ## Why Reactive, Not Timer-Based
//!
//! Apoptosis is timer-based (Phase 4, every ~1s) because death is predictable:
//! a connection either is stale or it isn't. Birth is unpredictable — it must
//! react to the unexpected.
//!
//! Neurogenesis is triggered by **prediction error** (surprise). When the
//! ForwardPassSystem compares a predicted value against reality and the
//! delta exceeds a threshold, this system spawns a new neuron to encode
//! the novel concept.
//!
//! ## The Leaky Bucket
//!
//! Raw prediction error is noisy. A single bad frame shouldn't spawn a neuron.
//! The surprise accumulator integrates error over time and decays naturally:
//!
//! ```text
//! accumulator[t+1] = (accumulator[t] + error_delta) * decay_rate
//!
//! threshold_breach → spawn neuron → accumulator resets to 0
//! ```
//!
//! This means:
//! - Transient noise (single spike) → decays away, no spawn
//! - Sustained novelty (persistent error) → accumulator fills → spawn
//! - After spawn → accumulator resets → system must learn before next spawn
//!
//! ## Integration Point
//!
//! Neurogenesis does NOT run in Phase 4. It runs inside the ForwardPassSystem's
//! prediction-correction loop — wherever the AGI compares its prediction against
//! observed reality. The DHT handler and transport layer also feed into it:
//! a CONSENSUS frame that fails to converge is a form of prediction error.

#![allow(missing_docs)]
use rand::RngCore;

use crate::components::{
    ActivationComponent, EntityId, PredictionComponent,
    SynapseComponent, ActivationMap, SynapseMap,
};

// ─── Constants — sensible defaults ──────────────────────────────

/// Default threshold: surprise must breach ~20% accumulated error to spawn
pub const DEFAULT_SPAWN_THRESHOLD: f32 = 0.2;

/// Default decay: retain 99% per tick (half-life ≈ 69 ticks)
pub const DEFAULT_DECAY_RATE: f32 = 0.99;

/// Max causal inputs to wire a new neuron to (prevents over-wiring)
pub const MAX_CAUSAL_INPUTS: usize = 16;

// ─── NeurogenesisSystem ────────────────────────────────────────

/// Tracks surprise and spawns new neurons when structural novelty is detected.
///
/// ## State Machine
///
/// ```text
/// IDLE → error arrives → ACCUMULATING → threshold breached
///       → SPAWNING (new EntityId + ActivationComponent + SynapseComponent)
///       → RESET (accumulator = 0) → IDLE
/// ```
pub struct NeurogenesisSystem {
    /// Integrated surprise signal (leaky bucket)
    pub surprise_accumulator: f32,
    /// Accumulator must reach this value to trigger a spawn
    pub spawn_threshold: f32,
    /// Multiplicative decay per track_error() call
    pub decay_rate: f32,
    /// Total neurons spawned since boot
    pub total_spawned: u64,
    /// Current engine tick (set externally each cycle)
    pub current_tick: u64,
}

impl Default for NeurogenesisSystem {
    fn default() -> Self {
        Self::new(DEFAULT_SPAWN_THRESHOLD, DEFAULT_DECAY_RATE)
    }
}

impl NeurogenesisSystem {
    pub fn new(spawn_threshold: f32, decay_rate: f32) -> Self {
        NeurogenesisSystem {
            surprise_accumulator: 0.0,
            spawn_threshold,
            decay_rate,
            total_spawned: 0,
            current_tick: 0,
        }
    }

    // Default constructor with sensible planetary-brain defaults.
    /// Monitors incoming prediction errors and fires spawns when the
    /// accumulated surprise breaches the threshold.
    ///
    /// Returns `true` if the threshold was breached — the caller should
    /// immediately call `spawn_neuron()`.
    ///
    /// ## Error Sources
    ///
    /// - `PredictionComponent.surprise()` — mathematical prediction error
    /// - Network frame CRC failures (single tick error spike)
    /// - Consensus divergence (peers disagree → systemic surprise)
    /// - DHT lookup failures (node unreachable → routing surprise)
    ///
    /// Each error source should feed its delta through here. Multiple
    /// sources accumulate together: a network failure + prediction error
    /// on the same tick is more surprising than either alone.
    pub fn track_error(&mut self, error_delta: f32) -> bool {
        self.surprise_accumulator += error_delta;

        if self.surprise_accumulator >= self.spawn_threshold {
            self.surprise_accumulator = 0.0; // full reset
            return true;
        }

        // Leaky decay — prevents noise from accumulating forever
        self.surprise_accumulator *= self.decay_rate;
        false
    }

    /// Spawn a new neuron and wire it to the causal upstream inputs.
    ///
    /// ## What Happens
    ///
    /// 1. Generate a new cryptographic EntityId (256-bit random)
    /// 2. Create an ActivationComponent with value 0.0
    /// 3. Wire SynapseComponent to `causal_inputs` with low initial weights
    /// 4. Return the new EntityId so the caller can register it elsewhere
    ///
    /// ## Causal Wiring
    ///
    /// The `causal_inputs` are the EntityIds of neurons that were upstream
    /// of the prediction error — specifically, the neurons whose activations
    /// fed into the erroneous prediction. This creates a directed edge from
    /// the new neuron TO each causal input, so the next ForwardPass includes
    /// the new neuron in the computation graph.
    pub fn spawn_neuron(
        activations: &mut ActivationMap,
        synapses: &mut SynapseMap,
        causal_inputs: Vec<EntityId>,
        current_tick: u64,
    ) -> EntityId {
        // Generate cryptographic random ID (256-bit)
        let mut id_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut id_bytes);
        let new_id = EntityId(id_bytes);

        // Initialize activation at 0 (resting potential)
        activations.insert(new_id, ActivationComponent {
            value: 0.0,
            last_updated_tick: current_tick,
        });

        // Limit wiring to prevent over-connection
        let inputs: Vec<EntityId> = causal_inputs.into_iter()
            .take(MAX_CAUSAL_INPUTS)
            .collect();
        let count = inputs.len();

        // Wire to causal upstream with low initial weights
        synapses.insert(new_id, SynapseComponent {
            target_entities: inputs,
            weights: vec![0.1; count],
            accumulated_gradients: vec![0.0; count],
        });

        new_id
    }

    /// Feed a PredictionComponent's surprise into the accumulator.
    ///
    /// This is the primary integration point for the ForwardPassSystem:
    /// after computing `PredictionComponent::new(predicted, actual)`,
    /// feed the result here.
    pub fn track_prediction(&mut self, prediction: &PredictionComponent) -> bool {
        self.track_error(prediction.surprise())
    }

    /// Convenience: feed raw prediction error directly.
    pub fn track_raw_error(&mut self, predicted: f32, actual: f32) -> bool {
        let pred = PredictionComponent::new(predicted, actual);
        self.track_prediction(&pred)
    }

    /// The current surprise level as a fraction of the threshold.
    /// 0.0 = no surprise, 1.0 = about to spawn.
    pub fn surprise_level(&self) -> f32 {
        (self.surprise_accumulator / self.spawn_threshold).min(1.0)
    }

    /// Whether the system is in a "curious" state — high but not
    /// yet critical surprise. Useful for logging/tuning.
    pub fn is_curious(&self) -> bool {
        self.surprise_level() > 0.5
    }

    /// Log current state
    pub fn print_status(&self) {
        if self.surprise_accumulator > 0.0 {
            let pct = self.surprise_level() * 100.0;
            eprintln!(
                "[NEUROGENESIS] surprise={:.2} ({:.0}% of threshold) spawned={}",
                self.surprise_accumulator, pct, self.total_spawned,
            );
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn eid(b: u8) -> EntityId { let mut a=[0u8;32]; a[31]=b; EntityId(a) }

    #[test]
    fn test_new_system_defaults() {
        let sys = NeurogenesisSystem::default();
        assert!((sys.spawn_threshold - 0.2).abs() < 0.001);
        assert!((sys.decay_rate - 0.99).abs() < 0.001);
        assert_eq!(sys.total_spawned, 0);
        assert_eq!(sys.surprise_accumulator, 0.0);
    }

    #[test]
    fn test_track_error_accumulates() {
        let mut sys = NeurogenesisSystem::new(0.5, 0.9);
        // First error: 0.3 → below threshold, decays by 0.9
        assert!(!sys.track_error(0.3));
        assert!((sys.surprise_accumulator - 0.27).abs() < 0.01);
        // Second error: 0.27 + 0.3 = 0.57 → above threshold
        assert!(sys.track_error(0.3));
        // Should have reset
        assert!((sys.surprise_accumulator - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_noise_decays_away() {
        let mut sys = NeurogenesisSystem::new(1.0, 0.5);
        // Tiny noise spike: 0.01 per tick for 20 ticks
        for _ in 0..20 {
            sys.track_error(0.01);
        }
        // Converges to steady-state of 0.01/(1-0.5)*0.5 = 0.01
        // With 20 ticks we're under 0.015
        assert!(sys.surprise_accumulator < 0.02);
    }

    #[test]
    fn test_spawn_neuron_generates_unique_id() {
        let mut acts = HashMap::new();
        let mut syns = HashMap::new();
        let inputs = vec![eid(1), eid(2)];

        let id1 = NeurogenesisSystem::spawn_neuron(&mut acts, &mut syns, inputs.clone(), 42);
        let id2 = NeurogenesisSystem::spawn_neuron(&mut acts, &mut syns, inputs, 43);

        // Different IDs
        assert_ne!(id1, id2);
        // Both registered
        assert_eq!(acts.len(), 2);
        assert_eq!(syns.len(), 2);
    }

    #[test]
    fn test_spawn_neuron_initial_values() {
        let mut acts = HashMap::new();
        let mut syns = HashMap::new();

        let id = NeurogenesisSystem::spawn_neuron(
            &mut acts, &mut syns, vec![eid(1), eid(2), eid(3)], 100,
        );

        let act = &acts[&id];
        assert!((act.value - 0.0).abs() < 0.001);
        assert_eq!(act.last_updated_tick, 100);

        let syn = &syns[&id];
        assert_eq!(syn.target_entities.len(), 3);
        assert_eq!(syn.weights.len(), 3);
        assert_eq!(syn.accumulated_gradients.len(), 3);
        for w in &syn.weights {
            assert!((*w - 0.1).abs() < 0.001);
        }
    }

    #[test]
    fn test_track_prediction() {
        let mut sys = NeurogenesisSystem::new(1.0, 0.9);
        // Perfect prediction → no surprise
        let p = PredictionComponent::new(0.5, 0.5);
        assert!(!sys.track_prediction(&p));
        assert!((sys.surprise_accumulator - 0.0).abs() < 0.001);

        // Terrible prediction → some surprise but decays
        let p2 = PredictionComponent::new(0.0, 10.0);
        assert!(!sys.track_prediction(&p2));
        assert!(sys.surprise_accumulator > 0.0);
    }

    #[test]
    fn test_raw_error() {
        let mut sys = NeurogenesisSystem::new(1.0, 0.9);
        assert!(!sys.track_raw_error(0.0, 10.0));
        assert!(sys.surprise_accumulator > 0.0);
    }

    #[test]
    fn test_surprise_level_clamped() {
        let mut sys = NeurogenesisSystem::new(0.5, 0.9);
        sys.surprise_accumulator = 2.0; // way above threshold
        assert!((sys.surprise_level() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_is_curious() {
        let mut sys = NeurogenesisSystem::new(1.0, 0.9);
        assert!(!sys.is_curious());
        sys.surprise_accumulator = 0.6;
        assert!(sys.is_curious());
    }
}
