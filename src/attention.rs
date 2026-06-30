//! Attention-like routing mechanisms for distributed neural networks.
//!
//! Routes pre-synaptic (input) activations to post-synaptic (output)
//! neurons using one of several mechanisms:
//!
//! * **HebbianSum** — plain weighted sum using existing weights.
//! * **HedgedAttention** — learned query/key projections with a value
//!   vector that gates the contribution of each pre-neuron.
//! * **GatedRouting** — a learned scalar gate applied to the
//!   pre-activation sum.
//! * **MoERouting** — mixture-of-experts style top-K selection.
//!
//! # Thread safety
//!
//! All routing structs are `Sync` because they contain no interior
//! mutability and all methods take `&self`.

use std::collections::HashMap;

use crate::components::EntityId;

// ─── HedgedAttention ────────────────────────────────────────────

/// Learned query/key projection attention mechanism.
///
/// For a pair `(pre_j, post_i)` with weight `w_ij`, the attention
/// score is computed as:
///
/// ```text
/// score_ij = softmax( (W_q·post_i)ᵀ · (W_k·pre_j) / sqrt(d) )
/// output_i = sum_j score_ij · v_j · w_ij · pre_j
/// ```
///
/// where `v_j` is a learned value vector element for pre-neuron `j`.
///
/// For simplicity in a distributed setting, the projections are
/// stored as flat `Vec<f32>` vectors rather than full matrices.
/// The query/key dimension is `query_dim` (= `key_dim`).
#[derive(Debug, Clone)]
pub struct HedgedAttention {
    /// Dimension of the query/key space.
    pub query_dim: usize,
    /// Dimension of the key space (same as query_dim in practice).
    pub key_dim: usize,
    /// Query projection weights, length `query_dim`.
    pub w_q: Vec<f32>,
    /// Key projection weights, length `key_dim`.
    pub w_k: Vec<f32>,
    /// Value weights, length = number of pre-neurons (set dynamically).
    pub v: Vec<f32>,
}

impl HedgedAttention {
    /// Create a new attention mechanism with zero-initialised projections.
    pub fn new(query_dim: usize, key_dim: usize) -> Self {
        HedgedAttention {
            query_dim,
            key_dim,
            w_q: vec![0.0; query_dim],
            w_k: vec![0.0; key_dim],
            v: Vec::new(),
        }
    }

    /// Create from explicit parameters.
    pub fn from_parts(
        query_dim: usize,
        key_dim: usize,
        w_q: Vec<f32>,
        w_k: Vec<f32>,
        v: Vec<f32>,
    ) -> Self {
        HedgedAttention {
            query_dim,
            key_dim,
            w_q,
            w_k,
            v,
        }
    }
}

// ─── GatedRouting ───────────────────────────────────────────────

/// A learned scalar gate applied to the aggregate pre-activation.
///
/// The gate is `sigmoid(w_g · pre_sum + b_g)` where `w_g` is a
/// learned weight vector (one per pre-neuron), and `b_g` is a scalar
/// bias.
#[derive(Debug, Clone)]
pub struct GatedRouting {
    /// Learned gate weights, one per pre-neuron.
    pub w_g: Vec<f32>,
    /// Gate bias.
    pub b_g: f32,
}

impl GatedRouting {
    /// Create a new gate with zero-initialised weights.
    pub fn new(n_pre: usize) -> Self {
        GatedRouting {
            w_g: vec![0.0; n_pre],
            b_g: 0.0,
        }
    }

    /// Compute the gate value: `sigmoid(sum_j w_g_j · pre_j + b_g)`.
    pub fn gate(&self, pre_activations: &HashMap<EntityId, f32>) -> f32 {
        let mut sum = self.b_g;
        for (j, w) in self.w_g.iter().enumerate() {
            if let Some(&pre_val) = pre_activations.get(&EntityId([j as u8; 32])) {
                sum += w * pre_val;
            }
        }
        sigmoid(sum)
    }
}

// ─── RoutingMechanism ───────────────────────────────────────────

/// Which routing strategy to use for aggregating pre-synaptic
/// activations into post-synaptic neurons.
#[derive(Debug, Clone)]
pub enum RoutingMechanism {
    /// Plain Hebbian weighted sum (identity routing).
    HebbianSum,
    /// Hedged attention with learned projections.
    HedgedAttention(HedgedAttention),
    /// Learned scalar gating.
    GatedRouting(GatedRouting),
    /// Mixture-of-Experts: keep only top-`k` contributors per
    /// post-neuron.
    MoERouting {
        /// Number of top pre-neurons to keep.
        top_k: usize,
    },
}

// ─── Main routing entry point ────────────────────────────────────

/// Route `pre_activations` through the given mechanism to produce
/// aggregated input activations for each post-synaptic neuron.
///
/// # Arguments
///
/// * `mechanism` — which routing strategy to use.
/// * `post_activations` — current activations of post-synaptic
///   neurons (may be empty).
/// * `pre_activations` — current activations of pre-synaptic neurons.
/// * `weights` — synaptic weights keyed by `(pre_id, post_id)`.
///
/// # Returns
///
/// A `HashMap<EntityId, f32>` mapping each **post-synaptic** neuron
/// ID to its aggregated input value.
pub fn route(
    mechanism: &RoutingMechanism,
    post_activations: &HashMap<EntityId, f32>,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    match mechanism {
        RoutingMechanism::HebbianSum => route_hebbian_sum(pre_activations, weights),
        RoutingMechanism::HedgedAttention(attn) => {
            route_hedged_attention(attn, post_activations, pre_activations, weights)
        }
        RoutingMechanism::GatedRouting(gate) => route_gated(gate, pre_activations, weights),
        RoutingMechanism::MoERouting { top_k } => route_moe(*top_k, pre_activations, weights),
    }
}

// ─── Routing implementations ─────────────────────────────────────

/// Hebbian sum: for each post-neuron, sum `w_ij · pre_j` over all
/// pre-neurons `j`.
fn route_hebbian_sum(
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    let mut result: HashMap<EntityId, f32> = HashMap::new();
    for ((pre, post), &w) in weights {
        if let Some(&pre_val) = pre_activations.get(pre) {
            *result.entry(*post).or_insert(0.0) += w * pre_val;
        }
    }
    result
}

/// Hedged attention routing.
fn route_hedged_attention(
    attn: &HedgedAttention,
    post_activations: &HashMap<EntityId, f32>,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    let mut result: HashMap<EntityId, f32> = HashMap::new();

    if attn.w_q.is_empty() || attn.w_k.is_empty() {
        // Fall back to Hebbian sum if projections are not set up.
        return route_hebbian_sum(pre_activations, weights);
    }

    // Group weights by post-neuron.
    let mut post_to_pre: HashMap<EntityId, Vec<(EntityId, f32)>> = HashMap::new();
    for ((pre, post), &w) in weights {
        post_to_pre.entry(*post).or_default().push((*pre, w));
    }

    for (post_id, pre_list) in &post_to_pre {
        // Compute query vector for this post-neuron.
        let post_val = post_activations.get(post_id).copied().unwrap_or(0.0);
        let mut query: Vec<f32> = attn.w_q.iter().map(|&w| w * post_val).collect();

        // Extend query to key_dim if needed.
        while query.len() < attn.key_dim {
            query.push(0.0);
        }
        query.truncate(attn.key_dim);

        // Compute scores and softmax normalisation.
        let mut scores: Vec<(usize, f32)> = Vec::with_capacity(pre_list.len());
        let mut max_score = f32::MIN;
        for (idx, (pre_id, w)) in pre_list.iter().enumerate() {
            let pre_val = pre_activations.get(pre_id).copied().unwrap_or(0.0);
            // Compute key vector for this pre-neuron.
            let key: Vec<f32> = attn.w_k.iter().map(|&wk| wk * pre_val).collect();

            // Dot product similarity.
            let dot: f32 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
            let score = dot / (attn.key_dim as f32).sqrt();
            scores.push((idx, score));
            if score > max_score {
                max_score = score;
            }
            let _ = w;
        }

        // Softmax with numerical stability.
        let mut exp_sum = 0.0_f32;
        for (_, score) in &scores {
            exp_sum += (score - max_score).exp();
        }
        let inv_exp_sum = if exp_sum > 0.0 { 1.0 / exp_sum } else { 0.0 };

        // Compute weighted sum for this post-neuron.
        let mut total = 0.0_f32;
        for (idx, score) in &scores {
            let (pre_id, w) = &pre_list[*idx];
            let pre_val = pre_activations.get(pre_id).copied().unwrap_or(0.0);
            let attention_weight = (score - max_score).exp() * inv_exp_sum;

            // Value weight for this pre-neuron (fallback to 1.0 if out of range).
            let v_j = attn.v.get(*idx).copied().unwrap_or(1.0);
            total += attention_weight * v_j * w * pre_val;
        }

        result.insert(*post_id, total);
    }

    result
}

/// Gated routing: gate the Hebbian sum with a learned scalar.
fn route_gated(
    gate: &GatedRouting,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    let hebbian = route_hebbian_sum(pre_activations, weights);
    let gate_val = gate.gate(pre_activations);

    // Apply the gate to every post-neuron.
    hebbian
        .into_iter()
        .map(|(post_id, val)| (post_id, val * gate_val))
        .collect()
}

/// Mixture-of-Experts routing: for each post-neuron, keep only the
/// top-`k` pre-neuron contributions.
fn route_moe(
    top_k: usize,
    pre_activations: &HashMap<EntityId, f32>,
    weights: &HashMap<(EntityId, EntityId), f32>,
) -> HashMap<EntityId, f32> {
    // Build per-post-neuron lists of (contribution, pre_id).
    let mut post_contributions: HashMap<EntityId, Vec<(f32, EntityId)>> = HashMap::new();
    for ((pre, post), &w) in weights {
        if let Some(&pre_val) = pre_activations.get(pre) {
            let contrib = w * pre_val;
            post_contributions
                .entry(*post)
                .or_default()
                .push((contrib, *pre));
        }
    }

    let mut result = HashMap::new();
    for (post_id, mut contributions) in post_contributions {
        // Sort by |contribution| descending.
        contributions.sort_unstable_by(|a, b| {
            b.0.abs()
                .partial_cmp(&a.0.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        contributions.truncate(top_k);

        let sum: f32 = contributions.iter().map(|(c, _)| c).sum();
        result.insert(post_id, sum);
    }

    result
}

// ─── Math helpers ────────────────────────────────────────────────

/// Logistic sigmoid.
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(v: u8) -> EntityId {
        EntityId([v; 32])
    }

    #[test]
    fn test_hebbian_sum_basic() {
        let mut pre = HashMap::new();
        pre.insert(eid(1), 0.5);
        pre.insert(eid(2), 1.0);

        let mut weights = HashMap::new();
        weights.insert((eid(1), eid(3)), 0.8);
        weights.insert((eid(2), eid(3)), 0.2);
        weights.insert((eid(1), eid(4)), 0.5);

        let result = route(
            &RoutingMechanism::HebbianSum,
            &HashMap::new(),
            &pre,
            &weights,
        );

        // post 3: 0.5*0.8 + 1.0*0.2 = 0.6
        assert!((result.get(&eid(3)).copied().unwrap_or(0.0) - 0.6).abs() < 1e-6);
        // post 4: 0.5*0.5 = 0.25
        assert!((result.get(&eid(4)).copied().unwrap_or(0.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_hebbian_sum_empty() {
        let result = route(
            &RoutingMechanism::HebbianSum,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn test_hedged_attention() {
        let attn = HedgedAttention::new(4, 4);
        let mut pre = HashMap::new();
        pre.insert(eid(1), 1.0);
        pre.insert(eid(2), 0.5);

        let mut weights = HashMap::new();
        weights.insert((eid(1), eid(3)), 0.8);
        weights.insert((eid(2), eid(3)), 0.2);

        let result = route(
            &RoutingMechanism::HedgedAttention(attn),
            &HashMap::new(),
            &pre,
            &weights,
        );

        // With zero projections, it falls back to Hebbian sum.
        assert!((result.get(&eid(3)).copied().unwrap_or(0.0) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_gated_routing() {
        let gate = GatedRouting::new(2);
        let mut pre = HashMap::new();
        pre.insert(eid(1), 1.0);
        pre.insert(eid(2), 0.5);

        let mut weights = HashMap::new();
        weights.insert((eid(1), eid(3)), 0.8);
        weights.insert((eid(2), eid(3)), 0.2);

        let result = route(
            &RoutingMechanism::GatedRouting(gate),
            &HashMap::new(),
            &pre,
            &weights,
        );

        // Gate ~ 0.5 (since w_g are zero, only bias 0.0 -> sigmoid(0) = 0.5)
        let _hebbian_val = 0.8 * 1.0 + 0.2 * 0.5; // = 0.9
        let expected = 0.9 * 0.5;
        assert!((result.get(&eid(3)).copied().unwrap_or(0.0) - expected).abs() < 1e-6);
    }

    #[test]
    fn test_moe_routing() {
        let mut pre = HashMap::new();
        pre.insert(eid(1), 1.0);
        pre.insert(eid(2), 0.5);
        pre.insert(eid(3), 2.0);

        let mut weights = HashMap::new();
        weights.insert((eid(1), eid(4)), 0.1);
        weights.insert((eid(2), eid(4)), 0.8);
        weights.insert((eid(3), eid(4)), 0.5);

        // top-K = 2: contributions: 0.1*1.0=0.1, 0.8*0.5=0.4, 0.5*2.0=1.0
        // Keep 1.0 and 0.4
        let result = route(
            &RoutingMechanism::MoERouting { top_k: 2 },
            &HashMap::new(),
            &pre,
            &weights,
        );

        let val = result.get(&eid(4)).copied().unwrap_or(0.0);
        assert!((val - 1.4).abs() < 1e-6);
    }

    #[test]
    fn test_moe_top1() {
        let mut pre = HashMap::new();
        pre.insert(eid(1), 1.0);
        pre.insert(eid(2), 2.0);

        let mut weights = HashMap::new();
        weights.insert((eid(1), eid(3)), 0.5);
        weights.insert((eid(2), eid(3)), 0.1);

        // |0.5*1.0| = 0.5, |0.1*2.0| = 0.2 -> keep only first
        let result = route(
            &RoutingMechanism::MoERouting { top_k: 1 },
            &HashMap::new(),
            &pre,
            &weights,
        );

        assert!((result.get(&eid(3)).copied().unwrap_or(0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_sync_trait() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<HedgedAttention>();
        assert_sync::<GatedRouting>();
        assert_sync::<RoutingMechanism>();
    }
}
