# ADR 4: Hebbian STDP Learning (Not Backpropagation)

**Status:** Accepted  
**Date:** 2026-04 (initial)  
**Deciders:** Project author

---

## Context

Distributed learning systems typically use gradient descent with backpropagation — the standard approach for training neural networks. A node computes gradients, shares them with peers, and the collective descent follows a (noisy) gradient toward a lower-loss configuration.

Backpropagation has well-known properties: provable convergence for convex objectives, extensive hardware acceleration (GPU kernels), and a mature ecosystem (PyTorch, TensorFlow, JAX).

**The problem with backprop for decentralized P2P learning:**
- Backprop requires coordinated loss computation across the network — every node must agree on the forward pass before computing gradients
- Synchronization rounds create a hidden central dependency even in "decentralized" SGD
- Gradient exchange requires reliable delivery — a lost update changes the descent trajectory
- The parameter vector must be globally consistent, which conflicts with eventual consistency

Hebbian learning ("cells that fire together, wire together") is local: each synapse adjusts based only on the pre- and post-synaptic activations available at that synapse. No global gradient signal is needed. This maps naturally onto a P2P network where each node only has local information.

## Decision

Use **Hebbian STDP (Spike-Timing-Dependent Plasticity) as the primary learning rule**, with a prediction-error-driven adaptation mechanism.

The learning system has three components:

**1. STDP weight updates:**
```
Δw_ij = η · a_i · a_j
```
Where η = 0.01 (learning rate), a_i is pre-synaptic activation, a_j is post-synaptic activation.

**2. Weight decay:**
```
w_ij(t+1) = (w_ij(t) + Δw_ij) × λ
```
Where λ = 0.999 per tick — prevents unbounded weight growth and prunes unused connections naturally.

**3. Prediction error (surprise) driving neurogenesis:**
```
accumulator[t+1] = (accumulator[t] + |prediction - observation|) × 0.99
if accumulator > threshold (0.2): spawn new neuron, reset accumulator
```

This is a three-factor learning rule: the third factor (prediction error) modulates structural plasticity (birth and death of neurons) while STDP handles local weight updates.

## Consequences

**Positive:**
- Fully local updates — no global coordination required for weight changes
- Eventual consistency is natural — nodes learn different patterns and only converge on shared structure
- Noise-tolerant — missing a gossip update is equivalent to delayed learning, not a wrong gradient
- Biologically plausible — maps to known mechanisms of cortical plasticity
- Enables selective gossip — only "surprising" activations trigger gossip, reducing bandwidth
- Neurogenesis and apoptosis are first-class operations, not regularization tricks

**Negative:**
- No convergence proofs for the learning dynamics — "it works" is empirical, not theoretical
- Hebbian learning cannot solve the full range of tasks that backprop can (credit assignment over long chains is limited)
- Less hardware acceleration potential — STDP is not GPU-friendly in its current form
- Unknown scaling behavior — does the sparse representation saturate at some network size?
- Evaluation is harder — classification accuracy metrics don't directly apply to self-organizing representations

**When this choice is wrong:**
If the goal were "train a state-of-the-art image classifier on distributed hardware," backprop would be the correct choice. Hebbian STDP is chosen here because the research question is about *how* decentralized learning can work, not about achieving competitive accuracy on benchmarks.

## Related

- ADR-2: UDP transport — STDP's noise tolerance makes BestEffort delivery acceptable for gradient gossip
- ADR-6: Sparse gossip — mutation-weighted selection biases gossip toward "surprising" peers
- ADR-8: No persistent storage — the learning state is ephemeral; in-memory representation is sufficient for the research prototype
- Implementation: `src/hebbian.rs`, `src/forward_pass.rs`, `src/neurogenesis.rs`

## References

- Hebb, D. O. (1949). *The Organization of Behavior*. Wiley & Sons.
- Gerstner, W., et al. (1996). A neuronal learning rule for sub-millisecond temporal coding. *Nature*.
- Lillicrap, T. P., et al. (2020). Backpropagation and the brain. *Nature Reviews Neuroscience*.
- Krotov, D., & Hopfield, J. J. (2019). Unsupervised learning by competing hidden units. *PNAS*.
