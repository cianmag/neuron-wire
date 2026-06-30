#![deny(missing_docs)]

//! Meta-learning module — learned optimizers and hypernetworks.
//!
//! Provides [`LearnedOptimizer`] for per-synapse learning rates, [`HyperNet`]
//! for dynamic weight generation, and the [`MetaMethod`] enum unifying both
//! approaches for use inside the engine loop.

use std::collections::HashMap;

use crate::components::EntityId;

// ─── LearnedOptimizer ───────────────────────────────────────────

/// A learned optimizer that maintains per-synapse cell states and meta-gradients.
///
/// Inspired by MAML and Meta-SGD, this optimizer tracks a hidden state vector
/// for each synapse `(pre → post)` and uses it to modulate the weight update
/// beyond what a fixed SGD rule would produce.
#[derive(Debug, Clone)]
pub struct LearnedOptimizer {
    /// Hidden dimensionality of the cell state per synapse
    pub hidden_dim: usize,
    /// Number of unrolled steps before a meta-gradient is computed
    pub unroll_steps: u64,
    /// Meta-learning rate applied to outer-loop updates
    pub meta_lr: f32,
    /// Cell state for each synapse:  `(pre, post) -> hidden vector`
    cell_state: HashMap<(EntityId, EntityId), Vec<f32>>,
    /// Accumulated meta-gradient buffer (flattened across all parameters)
    meta_grad_buffer: Vec<f32>,
    /// Step counter for this optimizer
    step: u64,
}

impl LearnedOptimizer {
    /// Create a new learned optimizer.
    ///
    /// * `hidden_dim` — size of the per-synapse hidden state (default: 8)
    /// * `unroll_steps` — how many inner steps before an outer meta-update (default: 5)
    /// * `meta_lr` — outer-loop learning rate (default: 0.001)
    pub fn new(hidden_dim: usize, unroll_steps: u64, meta_lr: f32) -> Self {
        LearnedOptimizer {
            hidden_dim,
            unroll_steps,
            meta_lr,
            cell_state: HashMap::new(),
            meta_grad_buffer: Vec::new(),
            step: 0,
        }
    }

    /// Compute the weight update `Δw` for a single synapse.
    ///
    /// The update rule is:
    ///
    /// ```text
    /// Δw = meta_lr · f(grad, w, a_pre, a_post, cell_state)
    /// ```
    ///
    /// where `f` is a learned gating function over the current hidden state.
    pub fn update(
        &mut self,
        synapse_id: (EntityId, EntityId),
        grad: f32,
        w: f32,
        a_pre: f32,
        a_post: f32,
    ) -> f32 {
        let state = self
            .cell_state
            .entry(synapse_id)
            .or_insert_with(|| vec![0.0; self.hidden_dim]);

        // Simple learned gating: combine inputs into a scalar hidden update
        let input_vec = [grad, w, a_pre, a_post];
        let mut gate = 0.0f32;
        for (i, &val) in input_vec.iter().enumerate() {
            if i < state.len() {
                gate += state[i] * val;
            }
        }
        let delta_w = self.meta_lr * (grad + 0.01 * gate.tanh());

        // Update the cell state with a simple recurrent step
        let lr_state = 0.1;
        for i in 0..state.len().min(4) {
            state[i] += lr_state * (input_vec[i % input_vec.len()] - 0.5 * state[i]);
        }
        // Decay remaining dimensions
        for i in 4..state.len() {
            state[i] *= 0.999;
        }

        self.step += 1;
        delta_w
    }

    /// Collect the current cell-states into a flat meta-gradient vector.
    ///
    /// This is intended for an outer-loop optimizer (e.g. Adam) that updates the
    /// meta-parameters of this learned optimizer.
    pub fn meta_gradient(&self) -> Vec<f32> {
        // Flatten all cell states into one vector
        let mut grad = Vec::new();
        for (_syn_id, state) in &self.cell_state {
            grad.extend_from_slice(state);
        }
        grad
    }

    /// Reset all cell states and step counter.
    pub fn reset(&mut self) {
        self.cell_state.clear();
        self.meta_grad_buffer.clear();
        self.step = 0;
    }

    /// Number of synapses being tracked.
    pub fn num_synapses(&self) -> usize {
        self.cell_state.len()
    }
}

// Safety: `LearnedOptimizer` contains no interior mutability or
// non-Send/Sync types — all mutation is via `&mut self`.
unsafe impl Sync for LearnedOptimizer {}

impl Default for LearnedOptimizer {
    fn default() -> Self {
        LearnedOptimizer::new(8, 5, 0.001)
    }
}

// ─── HyperNet ──────────────────────────────────────────────────

/// A hyper-network that generates parameters or weight updates.
///
/// The hyper-network is itself a feed-forward network with parameters stored
/// as a flat vector.  Given an embedding (e.g. a synapse's context vector)
/// it produces an output vector (e.g. the weight delta for that synapse).
#[derive(Debug, Clone)]
pub struct HyperNet {
    /// Hidden layer dimensionality
    pub hidden_dim: usize,
    /// Output dimensionality
    pub output_dim: usize,
    /// Flattened parameters:  `[W1, b1, W2, b2]`
    /// W1: hidden_dim × input_dim,  b1: hidden_dim,
    /// W2: output_dim × hidden_dim, b2: output_dim
    pub params: Vec<f32>,
    input_dim: usize,
}

impl HyperNet {
    /// Create a new hyper-network.
    ///
    /// * `input_dim` — size of the input embedding
    /// * `hidden_dim` — hidden layer size
    /// * `output_dim` — output size
    pub fn new(input_dim: usize, hidden_dim: usize, output_dim: usize) -> Self {
        let param_count =
            hidden_dim * input_dim + hidden_dim + output_dim * hidden_dim + output_dim;
        HyperNet {
            hidden_dim,
            output_dim,
            params: vec![0.0; param_count],
            input_dim,
        }
    }

    /// Forward pass: compute output from an input embedding.
    ///
    /// Architecture:  `input → Linear(hidden_dim) → ReLU → Linear(output_dim) → output`
    pub fn forward(&self, input_embedding: &[f32]) -> Vec<f32> {
        let mut hidden = vec![0.0; self.hidden_dim];
        #[allow(unused_variables)]
        let w1_start = 0;
        let w1_end = self.hidden_dim * self.input_dim;
        let b1_start = w1_end;
        let w2_start = b1_start + self.hidden_dim;
        let w2_end = w2_start + self.output_dim * self.hidden_dim;
        let b2_start = w2_end;

        // First layer
        for i in 0..self.hidden_dim {
            let mut sum = 0.0;
            for j in 0..self.input_dim {
                let idx = i * self.input_dim + j;
                if idx < self.params.len() && j < input_embedding.len() {
                    sum += self.params[idx] * input_embedding[j];
                }
            }
            if b1_start + i < self.params.len() {
                sum += self.params[b1_start + i];
            }
            hidden[i] = sum.max(0.0); // ReLU
        }

        // Second layer
        let mut output = vec![0.0; self.output_dim];
        for i in 0..self.output_dim {
            let mut sum = 0.0;
            for j in 0..self.hidden_dim {
                let idx = w2_start + i * self.hidden_dim + j;
                if idx < self.params.len() && j < hidden.len() {
                    sum += self.params[idx] * hidden[j];
                }
            }
            if b2_start + i < self.params.len() {
                sum += self.params[b2_start + i];
            }
            output[i] = sum;
        }

        output
    }

    /// Total number of parameters in this hyper-network.
    pub fn num_params(&self) -> usize {
        self.params.len()
    }
}

unsafe impl Sync for HyperNet {}

// ─── MetaMethod ─────────────────────────────────────────────────

/// Which meta-learning strategy (if any) is active.
#[derive(Debug, Clone)]
pub enum MetaMethod {
    /// No meta-learning — standard Hebbian/STDP updates
    None,
    /// Hyper-network that generates weight deltas from context
    HyperNet(HyperNet),
    /// Learned optimizer with per-synapse cell states
    LearnedOptimizer(LearnedOptimizer),
}

impl MetaMethod {
    /// Compute a weight delta `Δw` using the active meta-learning strategy.
    ///
    /// When `MetaMethod::None` is active this returns `grad` unchanged (i.e. the
    /// caller's own learning rate determines the update).
    pub fn update(
        &mut self,
        synapse_id: (EntityId, EntityId),
        grad: f32,
        w: f32,
        a_pre: f32,
        a_post: f32,
    ) -> f32 {
        match self {
            MetaMethod::None => grad,
            MetaMethod::HyperNet(hn) => {
                // Build a context embedding from (grad, w, a_pre, a_post)
                let context = vec![grad / (grad.abs() + 1e-8), w, a_pre, a_post];
                let out = hn.forward(&context);
                out.first().copied().unwrap_or(grad)
            }
            MetaMethod::LearnedOptimizer(lo) => lo.update(synapse_id, grad, w, a_pre, a_post),
        }
    }

    /// Return a meta-gradient vector for outer-loop training.
    pub fn meta_gradient(&self) -> Vec<f32> {
        match self {
            MetaMethod::None => Vec::new(),
            MetaMethod::HyperNet(hn) => hn.params.clone(),
            MetaMethod::LearnedOptimizer(lo) => lo.meta_gradient(),
        }
    }

    /// Reset any internal state.
    pub fn reset(&mut self) {
        match self {
            MetaMethod::None => {}
            MetaMethod::HyperNet(_) => {}
            MetaMethod::LearnedOptimizer(lo) => lo.reset(),
        }
    }
}

impl Default for MetaMethod {
    fn default() -> Self {
        MetaMethod::None
    }
}

unsafe impl Sync for MetaMethod {}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learned_optimizer_update() {
        let mut opt = LearnedOptimizer::new(4, 3, 0.01);
        let e1 = EntityId([1u8; 32]);
        let e2 = EntityId([2u8; 32]);
        let dw = opt.update((e1, e2), 0.5, 0.1, 0.8, 0.3);
        // Should produce a non-zero delta
        assert!(dw.abs() > 0.0);
        assert_eq!(opt.num_synapses(), 1);
    }

    #[test]
    fn test_hypernet_forward() {
        let hn = HyperNet::new(4, 8, 1);
        let inp = vec![1.0, 0.5, -0.2, 0.0];
        let out = hn.forward(&inp);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn test_meta_method_none() {
        let mut mm = MetaMethod::None;
        let e1 = EntityId([0u8; 32]);
        let e2 = EntityId([1u8; 32]);
        let dw = mm.update((e1, e2), 0.5, 0.1, 1.0, 0.0);
        assert_eq!(dw, 0.5);
        assert!(mm.meta_gradient().is_empty());
    }

    #[test]
    fn test_reset_clears_state() {
        let mut opt = LearnedOptimizer::new(4, 3, 0.01);
        let e1 = EntityId([0u8; 32]);
        let e2 = EntityId([1u8; 32]);
        let _ = opt.update((e1, e2), 0.5, 0.1, 0.8, 0.3);
        assert_eq!(opt.num_synapses(), 1);
        opt.reset();
        assert_eq!(opt.num_synapses(), 0);
    }
}
