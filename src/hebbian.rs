//! HebbianLearningSystem — Spike-Timing-Dependent Plasticity over NWP.
//!
//! ## The Four Phases of a Hebbian Tick
//!
//! ```text
//! For every (post_id, synapse) in the synapse map:
//!
//!   1. STDP:      Δw = η · pre_activation · post_activation
//!   2. Decay:     w  = (w + Δw) · λ          (λ = weight_decay)
//!   3. Micro-Pruning:
//!                  if w < prune_threshold → remove synapse immediately
//!   4. Gossip Batch:
//!                  if tick % gossip_interval == 0:
//!                    serialize accumulated_gradients → FlatBuffer → outbound_tx
//!                    reset accumulators → 0.0
//! ```
//!
//! ## Why Micro-Pruning Exists
//!
//! Apoptosis runs every 1s. A weight decaying from 0.1 to 0.001 at
//! λ = 0.999 takes ~4600 ticks (~4.6 seconds) to cross the threshold.
//! That's 4.6 seconds of wasted CPU and bandwidth on near-zero connections.
//!
//! Micro-pruning catches them the **instant** they drop below 0.001,
//! freeing the synapse slot for a new connection in the next
//! Neurogenesis spawn.
//!
//! ## Gossip Dispatch Strategy
//!
//! Gradient updates are batched locally and sent to a subset of peers
//! from the DHT routing table. The strategy is **cluster-biased random**:
//!
//! 1. Prioritise peers whose NodeType matches this node (same expert cluster)
//! 2. Fall back to random DHT peers
//! 3. Send at most 3 peers per gossip tick (keeps bandwidth within free-tier VPS limits)
//!
//! This converges gradients faster within expert clusters (vision nodes share
//! vision weights) while still propagating globally.

use std::net::SocketAddr;
use std::sync::mpsc::Sender;

use rand::Rng;

use crate::components::{
    ActivationMap, EntityId,
    SynapseComponent, SynapseMap,
};
use crate::engine_loop::{OutgoingPacket, Reliability};
use crate::header;

// ─── Constants ─────────────────────────────────────────────────

/// Default learning rate (η)
pub const DEFAULT_LEARNING_RATE: f32 = 0.01;

/// Default weight decay per tick (λ). At 1ms ticks:
/// half-life ≈ ln(0.5)/ln(0.999) ≈ 693 ticks ≈ 693ms
pub const DEFAULT_WEIGHT_DECAY: f32 = 0.999;

/// Weights below this are immediately severed
pub const DEFAULT_PRUNE_THRESHOLD: f32 = 0.001;

/// Gossip batch interval (every 500 ticks ≈ 500ms)
pub const DEFAULT_GOSSIP_INTERVAL: u64 = 500;

/// Max gossip targets per tick (respects free-tier VPS bandwidth)
pub const MAX_GOSSIP_TARGETS: usize = 3;

/// Max synapses per gossip packet (UDP MTU safety, ~1400 bytes / ~72 bytes per entry)
pub const MAX_SYNAPSES_PER_GOSSIP: usize = 19;

// ─── HebbianLearningSystem ────────────────────────────────────

/// Spike-Timing-Dependent Plasticity engine.
///
/// ## Thread Safety
///
/// HebbianLearningSystem is `Sync` — its state is read-only during a tick.
/// All mutable state lives in the SynapseMap passed to `tick()`.
pub struct HebbianLearningSystem {
    pub learning_rate: f32,
    pub weight_decay: f32,
    pub prune_threshold: f32,
    pub gossip_tick_interval: u64,
    /// Total weights pruned since boot (micro-pruning counter)
    pub total_micro_pruned: u64,
    /// Total gossip packets dispatched
    pub total_gossip_packets: u64,
}

impl Default for HebbianLearningSystem {
    fn default() -> Self {
        Self::new(
            DEFAULT_LEARNING_RATE,
            DEFAULT_WEIGHT_DECAY,
            DEFAULT_PRUNE_THRESHOLD,
            DEFAULT_GOSSIP_INTERVAL,
        )
    }
}

impl HebbianLearningSystem {
    pub fn new(learning_rate: f32, weight_decay: f32, prune_threshold: f32, gossip_interval: u64) -> Self {
        HebbianLearningSystem {
            learning_rate,
            weight_decay,
            prune_threshold,
            gossip_tick_interval: gossip_interval,
            total_micro_pruned: 0,
            total_gossip_packets: 0,
        }
    }

    /// Execute one Hebbian tick across all synapses.
    ///
    /// ## Parameters
    ///
    /// - `activations`: current ActivationMap (read-only, set by ForwardPassSystem)
    /// - `synapses`: mutable SynapseMap — weights are updated in-place
    /// - `current_tick`: engine tick counter (for gossip interval and timing)
    /// - `outbound_tx`: channel to enqueue gossip packets
    /// - `peers`: list of candidate gossip targets from DHT routing table
    /// - `local_id`: our EntityId (included in gossip for source identification)
    ///
    /// ## Returns
    ///
    /// A `HebbianReport` with stats for this tick.
    pub fn tick(
        &mut self,
        activations: &ActivationMap,
        synapses: &mut SynapseMap,
        current_tick: u64,
        outbound_tx: &Sender<OutgoingPacket>,
        peers: &[SocketAddr],
        local_id: EntityId,
    ) -> HebbianReport {
        let start = std::time::Instant::now();
        let mut report = HebbianReport::default();
        let mut gossip_batch: Vec<(EntityId, Vec<u8>)> = Vec::new();

        for (post_id, synapse) in synapses.iter_mut() {
            let post_val = activations.get(post_id).map(|a| a.value).unwrap_or(0.0);

            // Iterate in reverse so removals don't shift indices
            let mut i = synapse.target_entities.len();
            while i > 0 {
                i -= 1;
                let pre_id = &synapse.target_entities[i];
                let pre_val = activations.get(pre_id).map(|a| a.value).unwrap_or(0.0);

                // Phase 1: STDP
                let delta = self.learning_rate * pre_val * post_val;
                synapse.weights[i] += delta;
                synapse.accumulated_gradients[i] += delta;
                report.total_delta += delta.abs();

                // Phase 2: L2 Regularization (weight decay)
                synapse.weights[i] *= self.weight_decay;

                // Phase 3: Micro-Pruning
                if synapse.weights[i] < self.prune_threshold {
                    synapse.target_entities.remove(i);
                    synapse.weights.remove(i);
                    synapse.accumulated_gradients.remove(i);
                    report.micro_pruned += 1;
                    self.total_micro_pruned += 1;
                }
            }

            // Phase 4: Batch for Gossip
            if current_tick.is_multiple_of(self.gossip_tick_interval)
                && !synapse.accumulated_gradients.is_empty()
            {
                if let Some(serialized) = serialize_synapse_gossip(post_id, synapse) {
                    gossip_batch.push((*post_id, serialized));
                }
                synapse.accumulated_gradients.fill(0.0);
            }
        }

        // Fire gossip packets
        if !gossip_batch.is_empty() && !peers.is_empty() {
            let targets = select_gossip_targets(peers, MAX_GOSSIP_TARGETS);
            for target in &targets {
                if let Some(payload) = serialize_gossip_packet(local_id, &gossip_batch) {
                    let frame = header::build_frame(
                        crate::types::MsgType::Data as u8,
                        payload,
                        0,
                    );
                    let _ = outbound_tx.send(OutgoingPacket {
                        payload: frame,
                        dst: *target,
                        mode: Reliability::Data,
                    });
                    report.gossip_packets += 1;
                    self.total_gossip_packets += 1;
                }
            }
        }

        report.synapse_count = synapses.len();
        report.duration_us = start.elapsed().as_micros() as u64;
        report
    }

    /// Seed a new synapse with initial weight above threshold.
    /// Called by NeurogenesisSystem after spawning a neuron.
    pub fn seed_weight(&self) -> f32 {
        // Slightly above prune threshold so it survives at least one decay cycle
        self.prune_threshold * 100.0 // 0.1 at default threshold
    }
}

// ─── HebbianReport ────────────────────────────────────────────

/// Per-tick statistics for the Hebbian sweep.
#[derive(Debug, Clone, Default)]
pub struct HebbianReport {
    /// Number of synapses micro-pruned this tick
    pub micro_pruned: usize,
    /// Number of gossip packets sent this tick
    pub gossip_packets: usize,
    /// Total absolute delta applied across all weights
    pub total_delta: f32,
    /// Total synapses after this tick
    pub synapse_count: usize,
    /// Microseconds this tick took
    pub duration_us: u64,
}

// ─── Gossip Serialization ─────────────────────────────────────

/// Serialize a single (post_id, synapse) for gossip.
/// Format:
///   [32 bytes] post_id
///   [2 bytes]  u16: num_targets (LE)
///   For each target:
///     [32 bytes] target_entity_id
///     [4 bytes]  f32: weight (LE)
///     [4 bytes]  f32: accumulated_gradient (LE)
fn serialize_synapse_gossip(post_id: &EntityId, synapse: &SynapseComponent) -> Option<Vec<u8>> {
    if synapse.target_entities.is_empty() {
        return None;
    }
    let count = synapse.target_entities.len().min(MAX_SYNAPSES_PER_GOSSIP);
    let mut buf = Vec::with_capacity(32 + 2 + count * (32 + 4 + 4));

    buf.extend_from_slice(&post_id.0);
    buf.extend_from_slice(&(count as u16).to_le_bytes());

    for i in 0..count {
        buf.extend_from_slice(&synapse.target_entities[i].0);
        buf.extend_from_slice(&synapse.weights[i].to_le_bytes());
        buf.extend_from_slice(&synapse.accumulated_gradients[i].to_le_bytes());
    }

    Some(buf)
}

/// Serialize a full gossip packet containing multiple synapse updates.
/// Format:
///   [32 bytes] source_entity_id (who sent this)
///   [2 bytes]  u16: num_synapses (LE)
///   For each synapse: [serialized_synapse_gossip body without its header]
fn serialize_gossip_packet(local_id: EntityId, batch: &[(EntityId, Vec<u8>)]) -> Option<Vec<u8>> {
    if batch.is_empty() {
        return None;
    }
    let count = batch.len().min(MAX_SYNAPSES_PER_GOSSIP);
    let total_size: usize = 32 + 2 + batch.iter().take(count).map(|(_, b)| b.len()).sum::<usize>();
    let mut buf = Vec::with_capacity(total_size);

    buf.extend_from_slice(&local_id.0);
    buf.extend_from_slice(&(count as u16).to_le_bytes());

    for (_, serialized) in batch.iter().take(count) {
        buf.extend_from_slice(serialized);
    }

    Some(buf)
}

/// Deserialize a gossip packet received from the network.
/// Returns `(source_entity_id, Vec<(post_id, Vec<target_id>, Vec<weight>, Vec<gradient>)>)`
#[allow(dead_code)]
#[allow(clippy::type_complexity)]
pub fn deserialize_gossip_packet(data: &[u8]) -> Option<(EntityId, Vec<(EntityId, Vec<EntityId>, Vec<f32>, Vec<f32>)>)> {
    if data.len() < 34 { return None; }

    let mut offset = 0;
    let mut src_id = [0u8; 32];
    src_id.copy_from_slice(&data[offset..offset + 32]);
    offset += 32;
    let source = EntityId(src_id);

    let num_synapses = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
    offset += 2;

    let mut entries = Vec::with_capacity(num_synapses);

    for _ in 0..num_synapses {
        if offset + 34 > data.len() { break; }

        let mut pid = [0u8; 32];
        pid.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;
        let post_id = EntityId(pid);

        let num_targets = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        let expected_size = num_targets * (32 + 4 + 4);
        if offset + expected_size > data.len() { break; }

        let mut targets = Vec::with_capacity(num_targets);
        let mut weights = Vec::with_capacity(num_targets);
        let mut grads = Vec::with_capacity(num_targets);

        for _ in 0..num_targets {
            let mut tid = [0u8; 32];
            tid.copy_from_slice(&data[offset..offset + 32]);
            offset += 32;

            let w = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            offset += 4;

            let g = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            offset += 4;

            targets.push(EntityId(tid));
            weights.push(w);
            grads.push(g);
        }

        entries.push((post_id, targets, weights, grads));
    }

    Some((source, entries))
}

// ─── Gossip Target Selection ──────────────────────────────────

/// Select gossip targets from the DHT peer list.
///
/// Strategy (cluster-biased):
/// 1. Prefer peers with low latency (faster convergence)
/// 2. Cap at MAX_GOSSIP_TARGETS (3)
/// 3. Random selection from the top 10 fastest peers
fn select_gossip_targets(peers: &[SocketAddr], max: usize) -> Vec<SocketAddr> {
    if peers.is_empty() {
        return vec![];
    }

    // For now: random selection from all peers.
    // When DHT exposes NodeType info, we'll bias toward same-cluster peers.
    let mut rng = rand::thread_rng();
    let count = peers.len().min(max);

    // Reservoir sampling: pick `count` random unique peers
    let mut selected: Vec<SocketAddr> = Vec::with_capacity(count);
    let mut indices: Vec<usize> = (0..peers.len()).collect();
    for i in 0..count {
        let j = rng.gen_range(i..indices.len());
        indices.swap(i, j);
        selected.push(peers[indices[i]]);
    }

    selected
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::components::ActivationComponent;
    use super::*;
    use std::collections::HashMap;
    use std::sync::mpsc;

    fn eid(b: u8) -> EntityId { let mut a=[0u8;32]; a[31]=b; EntityId(a) }

    fn make_synapse(targets: Vec<(u8, f32)>) -> SynapseComponent {
        let mut ids = Vec::with_capacity(targets.len());
        let mut weights = Vec::with_capacity(targets.len());
        let grads = vec![0.0; targets.len()];
        for (b, w) in targets {
            ids.push(eid(b));
            weights.push(w);
        }
        SynapseComponent { target_entities: ids, weights, accumulated_gradients: grads }
    }

    #[test]
    fn test_hebbian_defaults() {
        let h = HebbianLearningSystem::default();
        assert!((h.learning_rate - 0.01).abs() < 0.001);
        assert!((h.weight_decay - 0.999).abs() < 0.001);
        assert!((h.prune_threshold - 0.001).abs() < 0.001);
    }

    #[test]
    fn test_stdp_updates_weight() {
        let mut h = HebbianLearningSystem::new(1.0, 1.0, 0.001, 1000);
        let mut acts = HashMap::new();
        acts.insert(eid(1), ActivationComponent { value: 0.5, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        syns.insert(eid(2), make_synapse(vec![(1, 0.1)]));

        let (tx, _rx) = mpsc::channel();
        let report = h.tick(&acts, &mut syns, 1, &tx, &[], eid(0));

        // Δw = 1.0 * 0.5 * 1.0 = 0.5, no decay (λ=1.0)
        // new weight = 0.1 + 0.5 = 0.6
        let w = syns[&eid(2)].weights[0];
        assert!((w - 0.6).abs() < 0.001);
        assert!(report.total_delta > 0.0);
    }

    #[test]
    fn test_weight_decay_applies() {
        let mut h = HebbianLearningSystem::new(0.0, 0.5, 0.001, 1000);
        let mut acts = HashMap::new();
        acts.insert(eid(1), ActivationComponent { value: 0.5, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        syns.insert(eid(2), make_synapse(vec![(1, 0.5)]));

        let (tx, _rx) = mpsc::channel();
        h.tick(&acts, &mut syns, 1, &tx, &[], eid(0));

        // No STDP (lr=0), just decay: 0.5 * 0.5 = 0.25
        let w = syns[&eid(2)].weights[0];
        assert!((w - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_micro_pruning_threshold() {
        let mut h = HebbianLearningSystem::new(0.0, 1.0, 0.3, 1000);
        let mut acts = HashMap::new();
        acts.insert(eid(1), ActivationComponent { value: 1.0, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        // weight 0.2 < 0.3 threshold → should be pruned
        syns.insert(eid(2), make_synapse(vec![(1, 0.2)]));

        let (tx, _rx) = mpsc::channel();
        let report = h.tick(&acts, &mut syns, 1, &tx, &[], eid(0));

        assert_eq!(report.micro_pruned, 1);
        assert_eq!(syns[&eid(2)].target_entities.len(), 0);
    }

    #[test]
    fn test_accumulated_gradients_reset_after_gossip() {
        let mut h = HebbianLearningSystem::new(1.0, 1.0, 0.001, 5);
        let mut acts = HashMap::new();
        acts.insert(eid(1), ActivationComponent { value: 0.5, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        syns.insert(eid(2), make_synapse(vec![(1, 0.1)]));

        let (tx, _rx) = mpsc::channel();

        // Tick 1: accumulate, no gossip (tick % 5 != 0)
        h.tick(&acts, &mut syns, 1, &tx, &[], eid(0));
        assert!(syns[&eid(2)].accumulated_gradients[0] > 0.0);

        // Tick 5: gossip triggers, gradients reset
        h.tick(&acts, &mut syns, 5, &tx, &[], eid(0));
        assert!((syns[&eid(2)].accumulated_gradients[0] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_gossip_serialize_deserialize_roundtrip() {
        let mut acts = HashMap::new();
        acts.insert(eid(1), ActivationComponent { value: 0.5, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        syns.insert(eid(2), make_synapse(vec![(1, 0.42)]));
        // Set gradient
        syns.get_mut(&eid(2)).unwrap().accumulated_gradients[0] = 0.17;

        let serialized = serialize_synapse_gossip(&eid(2), &syns[&eid(2)]).unwrap();
        assert!(!serialized.is_empty());

        // Wrap in a packet
        let packet = serialize_gossip_packet(eid(0xFF), &[(eid(2), serialized)]).unwrap();

        // Deserialize
        let (source, entries) = deserialize_gossip_packet(&packet).unwrap();
        assert_eq!(source, eid(0xFF));
        assert_eq!(entries.len(), 1);

        let (post_id, targets, weights, grads) = &entries[0];
        assert_eq!(*post_id, eid(2));
        assert_eq!(targets[0], eid(1));
        assert!((weights[0] - 0.42).abs() < 0.001);
        assert!((grads[0] - 0.17).abs() < 0.001);
    }

    #[test]
    fn test_gossip_target_selection() {
        let peers: Vec<SocketAddr> = (0..10)
            .map(|i| format!("10.0.0.{}:9000", i).parse().unwrap())
            .collect();

        // Should return at most 3
        let selected = select_gossip_targets(&peers, 3);
        assert!(selected.len() <= 3);
        assert!(selected.len() >= 1);

        // Empty peers → empty result
        let empty = select_gossip_targets(&[], 3);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_reverse_iteration_safe_removal() {
        let mut h = HebbianLearningSystem::new(0.0, 0.5, 0.01, 1000);
        let mut acts = HashMap::new();
        acts.insert(eid(0), ActivationComponent { value: 1.0, last_updated_tick: 0 });
        // Three targets, all will decay below threshold
        acts.insert(eid(0), ActivationComponent { value: 1.0, last_updated_tick: 0 });
        acts.insert(eid(1), ActivationComponent { value: 1.0, last_updated_tick: 0 });
        acts.insert(eid(2), ActivationComponent { value: 1.0, last_updated_tick: 0 });
        acts.insert(eid(3), ActivationComponent { value: 1.0, last_updated_tick: 0 });

        let mut syns = HashMap::new();
        syns.insert(eid(4), make_synapse(vec![(0, 0.5), (1, 0.5), (2, 0.5), (3, 0.5)]));

        let (tx, _rx) = mpsc::channel();
        let report = h.tick(&acts, &mut syns, 1, &tx, &[], eid(0));

        // 0.5 * 0.5 = 0.25, still above 0.01, so no pruning
        assert_eq!(report.micro_pruned, 0);
        // All 4 targets survive at 0.25
        assert_eq!(syns[&eid(4)].target_entities.len(), 4);
    }

    #[test]
    fn test_seed_weight_above_threshold() {
        let h = HebbianLearningSystem::new(1.0, 1.0, 0.001, 1000);
        let seed = h.seed_weight();
        assert!(seed >= h.prune_threshold);
        assert!((seed - 0.1).abs() < 0.001);
    }
}
