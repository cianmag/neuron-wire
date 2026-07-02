# Paper 6: WAN Deployment of P2P Neural Learning — A Multi-Region Study

**Target venue:** arXiv + experimental systems venue
**Status:** Planning
**Estimated pages:** 10–12

---

## Abstract

All published results for neuron-wire run on localhost. This paper presents the first real-world WAN deployment of a P2P neural computation fabric across 3+ geographic regions. We deploy 20–50 nodes on free-tier VPS instances across North America, Europe, and Asia, and measure: (1) DHT convergence time under realistic internet latency, (2) gradient gossip throughput across transoceanic links, (3) churn tolerance when VPS instances are killed and restarted, and (4) end-to-end learning convergence across WAN. We compare WAN results against localhost simulation to build a "reality gap" model: a correction function that predicts WAN behavior from simulation parameters. This is the single most impactful experiment the project can run — it transforms neuron-wire from a simulated prototype into a real distributed system.

## Key Claims

1. **First real WAN deployment of P2P neural learning**
2. **Reality gap model** — predict WAN behavior from localhost simulation
3. **Operational lessons** — NAT traversal, firewalls, clock skew, bandwidth asymmetry

## Outline

1. Introduction
2. Deployment Architecture
3. Measurement Methodology
4. WAN Results
5. Reality Gap Model
6. Operational Lessons
7. Comparison to Simulation
8. Conclusion

## Data Needed

- [ ] VPS instances in 3+ regions
- [ ] NAT traversal implementation (STUN/TURN)
- [ ] Encryption (Noise Protocol)
- [ ] Multi-region benchmark results

## Prerequisites

- STUN-based NAT traversal (v0.3.0 milestone)
- Noise Protocol encryption (v0.3.0 milestone)
- At minimum: 3 free-tier VPS instances
