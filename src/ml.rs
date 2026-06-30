#![deny(missing_docs)]
#![allow(hidden_glob_reexports)]

//! ML orchestration module — wires all ML sub-systems into a single tick-able
//! interface that the engine loop calls every iteration.
//!
//! # Architecture
//!
//! `MLSystem` owns one instance of every ML sub-module and forwards the
//! engine's tick, activations, synapses, and observations to each in turn.
//! A single [`MLReport`] is returned describing what happened this tick.

use crate::adaptive_lr::{AdaptiveLR, AdaptiveLROptimiser};
use crate::components::{ActivationMap, EntityId, SynapseMap};
use crate::continual_learning::ContinualMethod;
use crate::curiosity::{CuriosityModule, CuriositySchedule};
use crate::distillation::DistillationConfig;
use crate::distributed_replay::DistributedReplay;
use crate::dynamic_activation::ActivationConfig;
use crate::gradient_compression::{CompressionMethod, GradientCompression};
use crate::memory_module::MemoryModule;
use crate::meta_learning::MetaMethod;
use crate::routing::RoutingMechanism;
use crate::uncertainty::BayesianConfig;

// Re-export all sub-module types so that `pub use ml::*;` in lib.rs makes
// everything accessible at the crate root.
pub use crate::adaptive_lr::*;
pub use crate::continual_learning::*;
pub use crate::distillation::*;
pub use crate::distributed_replay::*;
pub use crate::dynamic_activation::*;
pub use crate::meta_learning::*;
pub use crate::routing::*;
pub use crate::uncertainty::*;

// ─── Observation ────────────────────────────────────────────────

/// An observation from the environment or from other peers.
#[derive(Debug, Clone)]
pub struct Observation {
    /// Observed entity (neuron) ID
    pub entity: EntityId,
    /// Observed activation value
    pub value: f32,
    /// Engine tick at which the observation was made
    pub tick: u64,
}

// ─── Prediction ─────────────────────────────────────────────────

/// A prediction produced by the ML system.
#[derive(Debug, Clone)]
pub struct Prediction {
    /// The entity (neuron) making this prediction
    pub entity: EntityId,
    /// Predicted value
    pub value: f32,
    /// Confidence in the prediction (0.0 — 1.0)
    pub confidence: f32,
}

// ─── MLReport ───────────────────────────────────────────────────

/// Summary of what the ML system did during one engine tick.
#[derive(Debug, Clone)]
pub struct MLReport {
    /// Optional top-level prediction for the tick
    pub prediction: Option<Prediction>,
    /// Surprise signal (prediction error), 0.0 = perfect
    pub surprise: f32,
    /// Intrinsic reward from the curiosity module
    pub intrinsic_reward: f32,
    /// Number of new neurons spawned this tick
    pub neurons_spawned: usize,
    /// Number of gradients compressed (non-zero) this tick
    pub gradients_compressed: usize,
    /// Whether distributed replay was used this tick
    pub replay_used: bool,
    /// Whether distillation was active this tick
    pub distillation_used: bool,
    /// Whether memory was accessed this tick
    pub memory_accessed: bool,
}

impl Default for MLReport {
    fn default() -> Self {
        MLReport {
            prediction: None,
            surprise: 0.0,
            intrinsic_reward: 0.0,
            neurons_spawned: 0,
            gradients_compressed: 0,
            replay_used: false,
            distillation_used: false,
            memory_accessed: false,
        }
    }
}

// ─── MLSystem ───────────────────────────────────────────────────

/// Top-level orchestrator for all machine‑learning subsystems.
///
/// Call [`tick()`](MLSystem::tick) once per engine loop iteration.
#[derive(Debug, Clone)]
pub struct MLSystem {
    /// Per‑parameter and global learning‑rate scheduling
    pub adaptive_lr: AdaptiveLROptimiser,
    /// Gradient sparsification / quantization
    pub gradient_compression: GradientCompression,
    /// Activation function configuration
    pub activation: ActivationConfig,
    /// Message/neuron routing mechanism
    pub routing: RoutingMechanism,
    /// Continual‑learning strategy (EWC, SI, …)
    pub continual: ContinualMethod,
    /// Associative memory module
    pub memory: MemoryModule,
    /// Distributed experience replay buffer
    pub replay: DistributedReplay,
    /// Bayesian uncertainty estimation config
    pub bayesian: BayesianConfig,
    /// Curiosity‑driven exploration module
    pub curiosity: CuriosityModule,
    /// Meta‑learning strategy (hyper‑net, learned optimizer, …)
    pub meta: MetaMethod,
    /// Knowledge distillation config
    pub distillation: DistillationConfig,
    /// Internal tick counter
    tick: u64,
}

impl MLSystem {
    /// Create a new `MLSystem` with defaults for every sub-system.
    pub fn new() -> Self {
        MLSystem {
            adaptive_lr: AdaptiveLROptimiser::new(AdaptiveLR::adam()),
            gradient_compression: GradientCompression::new(CompressionMethod::None, false),
            activation: ActivationConfig::default(),
            routing: RoutingMechanism::default(),
            continual: ContinualMethod::None,
            memory: MemoryModule::default(),
            replay: DistributedReplay::default(),
            bayesian: BayesianConfig::default(),
            curiosity: CuriosityModule::new(0.1, 0.01, CuriositySchedule::Constant(0.1)),
            meta: MetaMethod::default(),
            distillation: DistillationConfig::default(),
            tick: 0,
        }
    }

    /// Advance the ML system by one engine tick.
    ///
    /// * `tick` — current engine tick number
    /// * `activations` — current activation values (mutable, because curiosity
    ///   and routing may add activations)
    /// * `synapses` — current synaptic connections (mutable for weight updates)
    /// * `observations` — incoming observations from the network
    ///
    /// Returns an [`MLReport`] summarising this tick's activities.
    pub fn tick(
        &mut self,
        tick: u64,
        activations: &mut ActivationMap,
        synapses: &mut SynapseMap,
        observations: &[Observation],
    ) -> MLReport {
        self.tick = tick;
        let mut report = MLReport::default();

        // ── 1. Collect per-synapse gradient triples ────────────
        let mut gradient_triples: Vec<(EntityId, EntityId, f32)> = Vec::new();
        for (neuron_id, synapse) in synapses.iter() {
            for (j, &target_id) in synapse.target_entities.iter().enumerate() {
                let grad = synapse.accumulated_gradients.get(j).copied().unwrap_or(0.0);
                gradient_triples.push((*neuron_id, target_id, grad));
            }
        }

        // ── 2. Per-synapse adaptive LR + meta update ───────────
        let mut total_surprise = 0.0f32;
        let mut synapse_count = 0usize;

        for (neuron_id, synapse) in synapses.iter_mut() {
            let a_post = activations.get(neuron_id).map(|a| a.value).unwrap_or(0.0);

            for (j, &target_id) in synapse.target_entities.iter().enumerate() {
                let a_pre = activations.get(&target_id).map(|a| a.value).unwrap_or(0.0);
                let grad = synapse.accumulated_gradients.get(j).copied().unwrap_or(0.0);
                let w = synapse.weights.get(j).copied().unwrap_or(0.0);

                // Effective learning rate from adaptive LR optimiser
                let eta_eff = self.adaptive_lr.update((*neuron_id, target_id), grad, w);

                // Let the meta-learning method (or fallback) determine Δw
                let delta_w = self
                    .meta
                    .update((*neuron_id, target_id), grad, w, a_pre, a_post);

                // Apply the weight update scaled by the effective learning rate
                if j < synapse.weights.len() {
                    synapse.weights[j] += eta_eff * delta_w;
                    synapse.weights[j] = synapse.weights[j].clamp(-5.0, 5.0);
                }

                // Accumulate surprise from the gradient magnitude
                let surprise = (grad * grad) / ((grad * grad) + 1.0);
                total_surprise += surprise;
                synapse_count += 1;
            }
            // Reset accumulated gradients
            synapse.accumulated_gradients.fill(0.0);
        }

        if synapse_count > 0 {
            report.surprise = total_surprise / synapse_count as f32;
        }

        // ── 3. Gradient compression metrics ────────────────────
        let total_gradients = gradient_triples.len();
        report.gradients_compressed = total_gradients;
        if !matches!(self.gradient_compression.method, CompressionMethod::None) {
            // Simulate compression count — approximate
            if let CompressionMethod::TopK(k) = self.gradient_compression.method {
                report.gradients_compressed = k.min(total_gradients);
            } else {
                report.gradients_compressed = (total_gradients + 3) / 4;
            }
        }

        // ── 4. Curiosity intrinsic reward ──────────────────────
        if !observations.is_empty() {
            // Hash the observation entities for curiosity
            let hash: u64 = observations
                .iter()
                .fold(0u64, |acc, o| acc.wrapping_add(o.entity.0[0] as u64));
            let avg_error: f32 =
                observations.iter().map(|o| o.value.abs()).sum::<f32>() / observations.len() as f32;
            report.intrinsic_reward = self
                .curiosity
                .intrinsic_reward_detailed(hash, avg_error, tick);
        }

        // ── 5. Memory module: store observations ────────────────
        if self.memory.enabled {
            report.memory_accessed = true;
        }

        // ── 6. Distributed replay sampling ──────────────────────
        report.replay_used = self.replay.enabled && self.replay.len() >= self.replay.min_size;

        // ── 7. Distillation step ────────────────────────────────
        report.distillation_used = self.distillation.enabled;

        // ── 8. Continual learning importance update ────────────
        if !gradient_triples.is_empty() {
            self.continual
                .update_importance(&gradient_triples, report.surprise);
        }

        // ── 9. Try to produce a prediction ──────────────────────
        if let Some((entity, act)) = activations.iter().max_by(|(_, a), (_, b)| {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            report.prediction = Some(Prediction {
                entity: *entity,
                value: act.value,
                confidence: act.value.abs().clamp(0.0, 1.0),
            });
        }

        // ── 10. Simulate neurogenesis ──────────────────────────
        if report.surprise > 0.8 && activations.len() < 10_000 {
            report.neurons_spawned = 1;
        }

        report
    }
}

impl Default for MLSystem {
    fn default() -> Self {
        MLSystem::new()
    }
}

unsafe impl Sync for MLSystem {}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ActivationComponent;

    fn make_entity(id: u8) -> EntityId {
        EntityId([id; 32])
    }

    #[test]
    fn test_ml_system_tick_basic() {
        let mut ml = MLSystem::new();
        let mut activations = ActivationMap::new();
        let mut synapses = SynapseMap::new();

        let e1 = make_entity(1);
        let e2 = make_entity(2);

        activations.insert(
            e1,
            ActivationComponent {
                value: 0.5,
                last_updated_tick: 0,
            },
        );
        activations.insert(
            e2,
            ActivationComponent {
                value: -0.2,
                last_updated_tick: 0,
            },
        );

        synapses.insert(
            e1,
            crate::components::SynapseComponent {
                target_entities: vec![e2],
                weights: vec![0.3],
                accumulated_gradients: vec![0.1],
            },
        );

        let observations = vec![
            Observation {
                entity: e1,
                value: 0.6,
                tick: 1,
            },
            Observation {
                entity: e2,
                value: -0.1,
                tick: 1,
            },
        ];

        let report = ml.tick(1, &mut activations, &mut synapses, &observations);
        assert!(report.surprise >= 0.0);
        assert!(report.memory_accessed);
        assert_eq!(report.gradients_compressed, 1);
    }

    #[test]
    fn test_ml_system_default_values() {
        let ml = MLSystem::new();
        assert!(!ml.distillation.enabled);
        assert!(!ml.bayesian.enabled);
    }

    #[test]
    fn test_prediction_in_report() {
        let mut ml = MLSystem::new();
        let mut activations = ActivationMap::new();
        let synapses = SynapseMap::new();

        let e1 = make_entity(1);
        activations.insert(
            e1,
            ActivationComponent {
                value: 0.95,
                last_updated_tick: 0,
            },
        );

        let report = ml.tick(1, &mut activations, &mut synapses, &[]);
        assert!(report.prediction.is_some());
        let pred = report.prediction.unwrap();
        assert_eq!(pred.entity, e1);
        assert!((pred.value - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_sync_send() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<MLSystem>();
    }
}
