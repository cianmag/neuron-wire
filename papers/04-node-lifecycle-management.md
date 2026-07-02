# Paper 4: Node Lifecycle Management — Churn-Tolerant Routing in P2P Neural Networks

**Target venue:** arXiv + DSN / SRDS
**Status:** Draft
**Estimated pages:** 10–12

---

## Abstract

P2P learning networks operate in environments where nodes join, leave, and fail continuously. Unlike file-sharing networks where churn means temporarily unavailable data, in learning networks node departure means lost gradients, broken routing paths, and — most critically — lost learned representations. We present a node lifecycle management system for decentralized neural computation that handles churn through four mechanisms: (1) latency-weighted Kademlia DHT with active failure detection, (2) gradient persistence through temporal weight decay (stale gradients are decayed, not dropped), (3) graceful apoptosis of orphaned neurons, and (4) death-spiral detection that warns when routing table collapse exceeds configurable thresholds. In experiments with 30% node failure at 15 seconds, the network fully converges within 8 seconds post-failure, and the routing table stabilizes within 12 seconds. We characterize the regime where churn causes irrecoverable information loss and show that our mechanisms maintain useful state above 70% node survival.

## Key Claims

1. **Churn-tolerant routing** — DHT recovers from 30% node failure within seconds
2. **Graceful gradient decay** — information from dead nodes fades, doesn't vanish
3. **Death-spiral detection** — prevents cascading routing table collapse

## Outline

1. Introduction
2. Node Lifecycle States
3. Failure Detection and Recovery
4. Gradient Persistence Model
5. Apoptosis Integration
6. Death-Spiral Detection
7. Evaluation
8. Related Work
9. Conclusion

## Data Needed

- [x] Node churn experiments (30% failure)
- [x] Recovery time measurements
- [ ] Comparison of death-spiral thresholds
- [ ] Churn rate vs information loss curves
