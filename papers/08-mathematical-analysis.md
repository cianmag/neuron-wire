# Paper 8: Mathematical Foundations of P2P Neural Computation

**Target venue:** arXiv + ITCS / SODA / COLT
**Status:** Planning
**Estimated pages:** 15–20

---

## Abstract

We present a formal mathematical model for decentralized neural computation over P2P networks. The model covers five integrated subsystems: (1) **DHT Routing** — the communication complexity of latency-weighted Kademlia, with O(log n) expected lookup hops under churn; (2) **UDP Transport** — reliability bounds for gradient-weight decay, with exponential convergence of stale gradient significance; (3) **Hebbian STDP** — convergence of the weight matrix to input covariance, with learning rate η and decay λ determining steady-state bounds; (4) **Neurogenesis** — steady-state neuron count bounded under stationary input via the surprise accumulator's exponential forgetting; (5) **Apoptosis** — cascading death probability under simultaneous node failures, with the death-spiral guard threshold γ bounding total routing table loss.

We derive:
- **Theorem 3.1** (DHT routing complexity): Expected lookup hops ≤ log₂(n) + 1 with probability 1 - O(1/n)
- **Theorem 4.1** (STDP convergence): ||W(t) - Σ||_F ≤ O(e^{-λt}) under batch input
- **Theorem 5.1** (Neurogenesis steady-state): E[N(t)] ≤ (S₀ + α·T/τ) / (α·T·δ) as t → ∞
- **Theorem 6.1** (Apoptosis cascade bound): P(network partition) ≤ exp(-γ·|R|/β)

## Key Claims

1. **First unified model of P2P neural computation**
2. **Provable bounds on convergence, growth, and failure modes**
3. **All theorems falsifiable** — each makes concrete numerical predictions

## Outline

1. Introduction
2. System Model
3. DHT Routing Complexity
4. STDP Convergence
5. Neurogenesis Steady-State
6. Apoptosis Cascade Bounds
7. Empirical Validation
8. Open Problems
9. Conclusion

## Status

- [x] Partial analysis in FORMAL_MODEL.md
- [x] STDP convergence proof sketched
- [ ] Full formal proofs for all theorems
- [ ] Empirical validation of each bound
