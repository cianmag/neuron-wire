//! ForwardPassSystem — Neural Computation Pipeline.
//!
//! ## What It Does
//!
//! The forward pass is the AGI equivalent of a heartbeat: every tick, it
//! propagates activation signals through the local synapse graph, producing
//! predictions that are compared against incoming observations.
//!
//! ## The Pipeline (per tick)
//!
//! ```text
//! 1. LEAK:         Decay all existing activations (prevents runaway firing)
//! 2. PROPAGATE:    For each neuron, compute Σ(weight × pre_activation)
//! 3. SQUASH:       Apply tanh to propagated signal (clamp to [-1, 1])
//! 4. COMPARE:      Compare prediction vs observation → surprise → Neurogenesis
//! 5. CLEAN:        Remove orphan activations (no synapses → dead)
//! ```
//!
//! ## Engine Loop Integration
//!
//! The engine loop calls ForwardPassSystem FIRST, then separately calls
//! HebbianLearningSystem for STDP updates + gossip. This keeps concerns
//! separated: ForwardPass handles activation flow, Hebbian handles weight
//! adaptation.

#![allow(missing_docs)]
use std::collections::HashMap;

use crate::components::{
    ActivationComponent, ActivationMap, EntityId, PredictionComponent, SynapseMap,
};
use crate::neurogenesis::NeurogenesisSystem;

// ─── Constants ─────────────────────────────────────────────────

/// Default activation leak per tick (neurons return to 0 without input)
pub const DEFAULT_ACTIVATION_LEAK: f32 = 0.95;

/// Default prediction horizon (how many ticks ahead we predict)
pub const DEFAULT_PREDICTION_HORIZON: f32 = 1.0;

/// Squashing function: tanh scales any real value to [-1, 1]
/// Biologically inspired: membrane potential can't go to infinity.
pub fn squash_tanh(x: f32) -> f32 {
    x.tanh()
}

// ─── ForwardPassReport ──────────────────────────────────────────

/// Per-tick statistics for the forward pass pipeline.
#[derive(Debug, Clone, Default)]
pub struct ForwardPassReport {
    /// Number of neurons that received activation updates
    pub neurons_updated: usize,
    /// Number of predictions made this tick
    pub predictions_made: usize,
    /// Number of surprise events fed to Neurogenesis
    pub surprise_events: usize,
    /// Total prediction error (sum of all absolute surprises)
    pub total_surprise: f32,
    /// Number of new neurons spawned by Neurogenesis
    pub neurons_spawned: usize,
    /// Number of orphaned activations cleaned up
    pub orphans_cleaned: usize,
    /// Microseconds this tick took
    pub duration_us: u64,
}

// ─── ForwardPassSystem ─────────────────────────────────────────

/// Orchestrates activation propagation every tick.
///
/// ## Thread Safety
///
/// ForwardPassSystem is stateless — all mutable state lives in the
/// ActivationMap, SynapseMap, and NeurogenesisSystem passed to `tick()`.
pub struct ForwardPassSystem {
    /// Activation leak: how much activation retains between ticks (0.95 = 5% decay)
    pub activation_leak: f32,
    /// Prediction horizon multiplier
    pub prediction_horizon: f32,
}

impl Default for ForwardPassSystem {
    fn default() -> Self {
        Self::new(DEFAULT_ACTIVATION_LEAK, DEFAULT_PREDICTION_HORIZON)
    }
}

impl ForwardPassSystem {
    pub fn new(activation_leak: f32, prediction_horizon: f32) -> Self {
        ForwardPassSystem {
            activation_leak,
            prediction_horizon,
        }
    }

    // Default constructor with sensible defaults.
    /// Executes one forward pass tick: decay activations, propagate through
    /// synapses, incorporate observations, and detect prediction errors for
    /// neurogenesis.
    ///
    /// - `activations`: current ActivationMap (mutated in-place with new values)
    /// - `synapses`: mutable SynapseMap (weights unchanged — Hebbian handles that)
    /// - `neuro`: mutable NeurogenesisSystem (may spawn neurons on surprise)
    /// - `current_tick`: engine tick counter
    /// - `observations`: incoming observed activations (keyed by EntityId)
    ///   These come from network events — remote neurons' activation values.
    ///
    /// ## Engine Loop Note
    ///
    /// The engine loop should call this first, then call HebbianLearningSystem::tick()
    /// separately with the real outbound channel and peer list for gossip.
    pub fn tick(
        &mut self,
        activations: &mut ActivationMap,
        synapses: &mut SynapseMap,
        neuro: &mut NeurogenesisSystem,
        current_tick: u64,
        observations: &HashMap<EntityId, f32>,
    ) -> ForwardPassReport {
        let start = std::time::Instant::now();
        let mut report = ForwardPassReport::default();

        // ── PHASE 1: DECAY EXISTING ACTIVATIONS ───────────────
        // Apply leak to all neurons (they naturally return to 0)
        for act in activations.values_mut() {
            act.value *= self.activation_leak;
        }

        // ── PHASE 2: PROPAGATE THROUGH SYNAPSE GRAPH ──────────
        // For each neuron that has outgoing synapses, add its weighted
        // activation to its downstream targets.
        let mut propagation_buffer: HashMap<EntityId, f32> = HashMap::new();

        for (pre_id, synapse) in synapses.iter() {
            let pre_val = activations.get(pre_id).map(|a| a.value).unwrap_or(0.0);

            for (i, post_id) in synapse.target_entities.iter().enumerate() {
                let contribution = pre_val * synapse.weights[i];
                *propagation_buffer.entry(*post_id).or_insert(0.0) += contribution;
                report.neurons_updated += 1;
            }
        }

        // ── PHASE 3: APPLY PROPAGATION + SQUASH ───────────────
        // Merge propagated values into the activation map with squashing.
        for (id, raw_value) in propagation_buffer.drain() {
            let entry = activations
                .entry(id)
                .or_insert_with(|| ActivationComponent {
                    value: 0.0,
                    last_updated_tick: current_tick,
                });
            entry.value = squash_tanh(entry.value + raw_value);
            entry.last_updated_tick = current_tick;
        }

        // ── PHASE 4: COMPARE OBSERVATIONS AGAINST PREDICTIONS ─
        // When we receive an observed activation from the network,
        // compare it against the local prediction for that neuron.
        // The difference is a surprise signal.
        for (observed_id, observed_value) in observations {
            if let Some(predicted) = activations.get(observed_id) {
                let prediction = PredictionComponent::new(predicted.value, *observed_value);
                let surprise = prediction.surprise();

                if surprise > 0.001 {
                    report.surprise_events += 1;
                    report.total_surprise += surprise;

                    // Feed into Neurogenesis
                    neuro.current_tick = current_tick;
                    if neuro.track_error(surprise) {
                        // Threshold breached — spawn a new neuron
                        // wired to the causal upstream (the neurons
                        // that predicted this observation)
                        let mut causal: Vec<EntityId> = Vec::with_capacity(16);
                        for (pre_id, synapse) in synapses.iter() {
                            if synapse.target_entities.contains(observed_id) {
                                causal.push(*pre_id);
                            }
                        }

                        let _new_id = NeurogenesisSystem::spawn_neuron(
                            activations,
                            synapses,
                            causal,
                            current_tick,
                        );
                        neuro.total_spawned += 1;
                        report.neurons_spawned += 1;
                    }
                }

                // Overwrite predicted activation with observed value
                // (reality-grounding)
                if let Some(act) = activations.get_mut(observed_id) {
                    act.value = *observed_value;
                    act.last_updated_tick = current_tick;
                }
            } else {
                // Unknown neuron observed — novel entity.
                // Register it locally with the observed value.
                activations.insert(
                    *observed_id,
                    ActivationComponent {
                        value: *observed_value,
                        last_updated_tick: current_tick,
                    },
                );

                // Novelty is surprising — feed into Neurogenesis
                neuro.current_tick = current_tick;
                let surprise = observed_value.abs() * 0.1; // scaled novelty
                if neuro.track_error(surprise) {
                    let _new_id = NeurogenesisSystem::spawn_neuron(
                        activations,
                        synapses,
                        vec![*observed_id],
                        current_tick,
                    );
                    neuro.total_spawned += 1;
                    report.neurons_spawned += 1;
                    report.surprise_events += 1;
                    report.total_surprise += surprise;
                }
            }
        }

        // ── PHASE 5: CLEAN UP DORMANT NEURONS ──────────────────
        // Remove activations for neurons that have no synapse entries
        // AND zero activation value — they're truly dormant orphans.
        // (Target neurons created by propagation are NOT removed —
        //  they have non-zero activation from the forward pass.)
        let dead_ids: Vec<EntityId> = activations
            .iter()
            .filter(|(id, act)| act.value.abs() < 0.001 && !synapses.contains_key(id))
            .map(|(id, _)| *id)
            .collect();
        report.orphans_cleaned = dead_ids.len();
        for id in dead_ids {
            activations.remove(&id);
        }

        report.duration_us = start.elapsed().as_micros() as u64;
        report
    }
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::SynapseComponent;
    use crate::neurogenesis::NeurogenesisSystem;
    use std::collections::HashMap;

    fn eid(b: u8) -> EntityId {
        let mut a = [0u8; 32];
        a[31] = b;
        EntityId(a)
    }

    fn make_activations() -> ActivationMap {
        let mut m = HashMap::new();
        m.insert(
            eid(1),
            ActivationComponent {
                value: 0.5,
                last_updated_tick: 0,
            },
        );
        m.insert(
            eid(2),
            ActivationComponent {
                value: 0.8,
                last_updated_tick: 0,
            },
        );
        m
    }

    fn make_synapses() -> SynapseMap {
        let mut m = HashMap::new();
        m.insert(
            eid(1),
            SynapseComponent {
                target_entities: vec![eid(3)],
                weights: vec![0.5],
                accumulated_gradients: vec![0.0],
            },
        );
        m.insert(
            eid(2),
            SynapseComponent {
                target_entities: vec![eid(3)],
                weights: vec![0.5],
                accumulated_gradients: vec![0.0],
            },
        );
        m
    }

    #[test]
    fn test_default_system() {
        let fp = ForwardPassSystem::default();
        assert!((fp.activation_leak - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_propagation_simple() {
        let mut fp = ForwardPassSystem::new(1.0, 1.0); // no leak
        let mut acts = make_activations();
        let mut syns = make_synapses();
        let mut neuro = NeurogenesisSystem::new(10.0, 0.5); // high threshold

        // Neuron 3 has inputs from 1 (0.5 × 0.5) and 2 (0.8 × 0.5)
        // Expected: 0.25 + 0.4 = 0.65
        // Neuron 3 isn't in the activation map yet — propagation_buffer creates it
        let report = fp.tick(&mut acts, &mut syns, &mut neuro, 1, &HashMap::new());

        // Neuron 3 should now exist with tanh(0.65) ≈ 0.571
        assert!(acts.contains_key(&eid(3)));
        let val = acts[&eid(3)].value;
        assert!((val - 0.65f32.tanh()).abs() < 0.01);
        assert!(report.neurons_updated > 0);
    }

    #[test]
    fn test_activation_leak_applies() {
        let mut fp = ForwardPassSystem::new(0.5, 1.0); // 50% leak
        let mut acts = make_activations();
        let mut syns = SynapseMap::new();
        let mut neuro = NeurogenesisSystem::default();

        fp.tick(&mut acts, &mut syns, &mut neuro, 1, &HashMap::new());

        assert!((acts[&eid(1)].value - 0.25).abs() < 0.01);
        assert!((acts[&eid(2)].value - 0.40).abs() < 0.01);
    }

    #[test]
    fn test_observation_drives_surprise() {
        let mut fp = ForwardPassSystem::new(1.0, 1.0);
        let mut acts = make_activations();
        let mut syns = make_synapses();
        let mut neuro = NeurogenesisSystem::new(5.0, 0.9); // high threshold

        // Observation: neuron 3 has value 1.0, but local predicts tanh(0.65) ≈ 0.57
        let mut obs = HashMap::new();
        obs.insert(eid(3), 1.0);

        let report = fp.tick(&mut acts, &mut syns, &mut neuro, 1, &obs);

        // Should have generated a surprise event
        assert!(report.total_surprise > 0.0);

        // Neuron 3 should be grounded to observed value
        assert!((acts[&eid(3)].value - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dead_neurons_cleaned() {
        let mut fp = ForwardPassSystem::new(1.0, 1.0);
        let mut acts = make_activations();
        // Set activation values to 0 so they're truly dormant
        for act in acts.values_mut() {
            act.value = 0.0;
        }
        let mut syns = SynapseMap::new(); // empty — all neurons are orphans
        let mut neuro = NeurogenesisSystem::new(10.0, 0.9);

        assert_eq!(acts.len(), 2);
        let report = fp.tick(&mut acts, &mut syns, &mut neuro, 1, &HashMap::new());

        // Dormant activations should be removed
        assert_eq!(acts.len(), 0);
        assert_eq!(report.orphans_cleaned, 2);
    }

    #[test]
    fn test_novel_observation_registers_neuron() {
        let mut fp = ForwardPassSystem::new(1.0, 1.0);
        let mut acts = make_activations();
        let mut syns = make_synapses();
        let mut neuro = NeurogenesisSystem::new(10.0, 0.9); // high threshold, no spawn

        // Observe a completely unknown neuron
        let mut obs = HashMap::new();
        obs.insert(eid(99), 0.5);

        let report = fp.tick(&mut acts, &mut syns, &mut neuro, 1, &obs);

        // Should NOT have spawned (threshold too high), but neuron 99 should be registered
        assert_eq!(report.neurons_spawned, 0);
        assert!(acts.contains_key(&eid(99)));
        assert!((acts[&eid(99)].value - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_squash_tanh_limits() {
        let big = squash_tanh(100.0);
        assert!((big - 1.0).abs() < 0.001);

        let neg = squash_tanh(-100.0);
        assert!((neg + 1.0).abs() < 0.001);

        let zero = squash_tanh(0.0);
        assert!((zero - 0.0).abs() < 0.001);
    }
}
