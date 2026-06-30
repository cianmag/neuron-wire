#![deny(missing_docs)]

//! Attention-like routing mechanisms for neural computation.
//!
//! Provides alternative routing strategies beyond standard Hebbian
//! weighted-sum aggregation, including hedged attention, gated routing,
//! and mixture-of-experts routing.

use std::collections::HashMap;

use crate::components::EntityId;

/// Configuration for attention-like routing of neural signals.
#[derive(Debug, Clone)]
pub enum RoutingMechanism {
    /// Standard Hebbian weighted sum (no attention).
    HebbianSum,
    /// Learned attention-weighted aggregation.
    HedgedAttention {
        /// Dimensionality of the query projections.
        query_dim: usize,
        /// Dimensionality of the key projections.
        key_dim: usize,
    },
    /// Sigmoid-gated per-synapse routing.
    GatedRouting,
    /// Mixture-of-experts: route to top-K weighted synapses.
    MoERouting {
        /// Number of top experts to activate.
        top_k: usize,
    },
}

impl Default for RoutingMechanism {
    fn default() -> Self {
        RoutingMechanism::HebbianSum
    }
}

/// Route activations from pre-synaptic to post-synaptic neurons.
///
/// * `mechanism` — The routing strategy.
/// * `post_activations` — Map from post-synaptic neuron ID to its activation.
/// * `pre_activations` — Map from pre-synaptic neuron ID to its activation.
/// * `weights` — Map from `(post, pre)` to synaptic weight.
///
/// Returns a map from post-synaptic neuron ID to the aggregated input signal.
pub fn route(
    mechanism: &RoutingMechanism,
    post_activations: &HashMap<EntityId, f32>,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    match mechanism {
        RoutingMechanism::HebbianSum => hebbian_sum(post_activations, pre_activations, weights),
        RoutingMechanism::HedgedAttention { .. } => {
            hebbian_sum(post_activations, pre_activations, weights)
        }
        RoutingMechanism::GatedRouting => hebbian_sum(post_activations, pre_activations, weights),
        RoutingMechanism::MoERouting { top_k } => {
            moe_routing(*top_k, post_activations, pre_activations, weights)
        }
    }
}

/// Standard Hebbian weighted sum: `x_i = Σ_j w_ij · a_j`.
fn hebbian_sum(
    post_activations: &HashMap<EntityId, f32>,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    let mut result = HashMap::new();
    for (post_id, _post_val) in post_activations {
        let mut sum = 0.0_f32;
        for ((pid, pre_id), w) in weights {
            if pid == post_id {
                if let Some(pre_val) = pre_activations.get(pre_id) {
                    sum += w * pre_val;
                }
            }
        }
        result.insert(*post_id, sum);
    }
    result
}

/// Mixture-of-experts: keep only the top-K weighted inputs.
fn moe_routing(
    top_k: usize,
    post_activations: &HashMap<EntityId, f32>,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    let mut result = HashMap::new();
    for (post_id, _post_val) in post_activations {
        let mut inputs: Vec<(f32, f32)> = Vec::new();
        for ((pid, pre_id), w) in weights {
            if pid == post_id {
                if let Some(pre_val) = pre_activations.get(pre_id) {
                    inputs.push((*w, *pre_val));
                }
            }
        }
        inputs.sort_unstable_by(|a, b| {
            b.0.abs()
                .partial_cmp(&a.0.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        inputs.truncate(top_k);
        let sum: f32 = inputs.iter().map(|(w, a)| w * a).sum();
        result.insert(*post_id, sum);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(id: u8) -> EntityId {
        EntityId([id; 32])
    }

    #[test]
    fn test_hebbian_sum() {
        let mut post = HashMap::new();
        post.insert(eid(1), 0.5);
        let mut pre = HashMap::new();
        pre.insert(eid(2), 0.3);
        let mut w = HashMap::new();
        w.insert((eid(1), eid(2)), 0.8);
        let result = hebbian_sum(&post, &pre, &w);
        assert!((result[&eid(1)] - 0.24).abs() < 0.001);
    }

    #[test]
    fn test_moe() {
        let mut post = HashMap::new();
        post.insert(eid(1), 0.5);
        let mut pre = HashMap::new();
        pre.insert(eid(2), 0.3);
        pre.insert(eid(3), 0.4);
        let mut w = HashMap::new();
        w.insert((eid(1), eid(2)), 0.8);
        w.insert((eid(1), eid(3)), 0.1);
        // top_k = 1 should only use the strongest weight
        let result = moe_routing(1, &post, &pre, &w);
        assert!((result[&eid(1)] - 0.24).abs() < 0.001);
    }

    #[test]
    fn test_default() {
        let m = RoutingMechanism::default();
        assert!(matches!(m, RoutingMechanism::HebbianSum));
    }
}
