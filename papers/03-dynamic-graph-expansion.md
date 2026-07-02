# Paper 3: Dynamic Graph Expansion — Surprise-Driven Neurogenesis in P2P Networks

**Target venue:** arXiv + NeurIPS Workshop / ICLR
**Status:** Draft
**Estimated pages:** 8–10

---

## Abstract

Biological neural networks grow and prune connections in response to experience. Artificial neural networks, by contrast, are born with a fixed architecture and never change it. We present a mechanism for dynamic graph expansion in decentralized neural networks where nodes autonomously spawn new neurons in response to sustained prediction error (surprise). A cumulative surprise accumulator triggers neurogenesis when it exceeds a configurable threshold, creating random-weight neurons that integrate into the existing topology via Hebbian plasticity. We prove that the steady-state neuron count is bounded under stationary input distributions and derive the expected time between birth events. In simulation, surprise-driven neurogenesis enables networks of 10 initial neurons to grow to task-appropriate sizes without centralized coordination, and apoptosis prunes inactive neurons to prevent unbounded growth.

## Key Claims

1. **Fully decentralized neurogenesis** — no central coordinator decides when to add capacity
2. **Surprise-driven growth** — neurons are born exactly where prediction error is highest
3. **Bounded steady state** — the system does not grow forever under stationary conditions

## Outline

1. Introduction
2. The Surprise Accumulator Model
3. Neurogenesis Algorithm
4. Apoptosis and Homeostasis
5. Theoretical Analysis (steady-state bounds)
6. Evaluation
7. Related Work
8. Conclusion

## Data Needed

- [x] Neurogenesis unit tests (8 tests)
- [x] Apoptosis unit tests (5 tests)
- [ ] End-to-end simulation showing network growth over time
- [ ] Steady-state bounds verification
