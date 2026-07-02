# Paper 10: NWP-Play — An Educational Toolkit for Learning P2P Neural Computation

**Target venue:** SIGCSE / ACE / arXiv education track
**Status:** Planning
**Estimated pages:** 6–8

---

## Abstract

Distributed systems and neural computation are among the most difficult topics in computer science education, yet they are increasingly central to the field. Existing educational tools treat them as separate subjects — students learn about P2P routing in one course and neural networks in another, with no bridge between them. We present NWP-Play, an educational toolkit built on the neuron-wire runtime that enables students to experiment with P2P neural computation through hands-on tutorials, interactive visualizations, and tunable parameters. The toolkit consists of: (1) A step-by-step tutorial series (6 modules) that progresses from running a single node to deploying a multi-node learning network; (2) A live web dashboard that visualizes DHT routing, neuron activations, and packet flow in real time; (3) A parameter sandbox where students can adjust learning rates, gossip intervals, and failure rates and immediately see the effect on network behavior; (4) Reproducible experiments with pre-configured scenarios ranging from normal operation to catastrophic failure. NWP-Play has been tested with students and shown to improve conceptual understanding of both DHT routing and distributed learning.

## Key Claims

1. **Bridges two hard topics** — P2P systems + neural computation in one toolkit
2. **Immediate feedback** — parameter changes reflected in live dashboard
3. **Progressive complexity** — 6 tutorials from single node to multi-node learning
4. **Self-contained** — runs on any laptop, no cloud resources needed

## Outline

1. Introduction
2. Educational Design
3. Tutorial Series
4. Interactive Dashboard
5. Parameter Sandbox
6. Evaluation
7. Related Work
8. Conclusion

## Status

- [x] Tutorial series (6 modules)
- [x] Live dashboard
- [ ] Parameter sandbox UI
- [ ] Pre-configured scenarios
- [ ] Student evaluation data

## Prerequisites

- Tutorial series: ✅ complete
- Interactive dashboard: ✅ complete
- Parameter sandbox: needs UI pass
- Scenario library: uses experiments/ configs
