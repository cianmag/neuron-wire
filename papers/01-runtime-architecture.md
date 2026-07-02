# Paper 1: Neuron-Wire — A Decentralized Runtime for Distributed Neural Computation

**Target venue:** arXiv (fast preprint) + HotOS / APSys (systems conference)
**Status:** Draft — expand from existing `PAPER.md`
**Estimated pages:** 12–16 (conference format), 20+ (arXiv)

---

## Abstract

We present neuron-wire, a decentralized adaptive runtime for large-scale distributed learning. Unlike federated learning (centralized coordinator), parameter-server architectures (single point of failure), or blockchain-based ML (crippling overhead), neuron-wire implements a purpose-built P2P fabric that combines modified Kademlia DHT routing, custom UDP transport with gradient-weight decay, and local Hebbian plasticity in a single-threaded non-blocking engine loop. Nodes discover each other autonomously, exchange activation gradients over unreliable networks, and adapt local connectivity through spike-timing-dependent plasticity — with no central coordination, no GPU cluster, and no data leaving the device. The implementation is 8,000+ lines of safe Rust with 110+ tests, zero compiler warnings, and a single-command reproducible benchmark suite. On a single laptop core, the engine sustains ~1 MHz tick rate idle and ~800 KHz under neural computation. In localhost clusters of 3–50 nodes, DHT convergence completes in O(log n) time with 100% success rate.

## Key Claims

1. **First purpose-built P2P neural computation runtime** — not an ML framework retrofitted for distribution, but a distributed systems protocol designed from the ground up for neural workloads
2. **Deterministic simulation** — every experiment is fully reproducible with `--paper-mode`, fixed seeds, frozen configs, and output CSVs
3. **Practical performance** — single-threaded, no async runtime, ~500 KB stripped binary, runs on any Linux x86-64 host
4. **Open source** — MIT/Apache-2.0, full CI/CD, contributor guide, community infrastructure

## Outline

1. **Introduction** (2 pages)
   - Problem: centralization in ML
   - Research question: can P2P neural computation work?
   - Contributions summary

2. **System Design** (4 pages)
   - 2.1 Engine Loop (single-threaded tick-based architecture)
   - 2.2 DHT Routing (latency-weighted Kademlia)
   - 2.3 UDP Transport (reliability tiers, gradient-weight decay)
   - 2.4 Neural Computation (ForwardPass + Hebbian STDP)
   - 2.5 Neurogenesis (surprise-driven neuron birth)
   - 2.6 Apoptosis (programmed neuron death)

3. **Implementation** (2 pages)
   - 3.1 Language and Dependencies (Rust + 8 crates)
   - 3.2 Code Organization (ECS architecture)
   - 3.3 Testing (110+ unit tests, 14 benchmarks, 7 integration, 8 proptest)

4. **Evaluation** (4 pages)
   - 4.1 Engine Performance (tick rate benchmarks)
   - 4.2 DHT Discovery (3-node, 10-node, 25-node, 50-node)
   - 4.3 Scaling Laws (convergence time, bandwidth vs nodes)
   - 4.4 Multi-trial Statistics (10 trials, confidence intervals)
   - 4.5 Failure Injection (node death, packet loss, network partitions)

5. **Comparison** (1 page)
   - Table vs federated learning, all-reduce, gossip SGD, blockchain, IPFS

6. **Limitations** (1 page)
   - No NAT traversal, no encryption, single-threaded, no persistence, no Byzantine tolerance

7. **Related Work** (1 page)

8. **Conclusion** (0.5 page)

## Data Needed

- [x] DHT convergence benchmarks (3/10/25/50 nodes)
- [x] Engine tick rate measurements
- [x] Bandwidth scaling data
- [ ] End-to-end learning demonstration (before/after weights)
- [ ] CPU/RAM profiling under load

## Writing Status

| Section | Status | Notes |
|---------|--------|-------|
| Abstract | ✅ Draft exists in PAPER.md | Needs update for v0.3.0 |
| Introduction | ✅ Rough draft | |
| System Design | ✅ Detailed in ARCHITECTURE.md | Needs condensation |
| Implementation | ✅ | Needs line count update |
| Evaluation | ⬜ Needs benchmarks run | Data exists for 3-node only |
| Comparison | ⬜ | Table exists in PAPER.md |
| Limitations | ✅ | |
| Related Work | ⬜ | Needs literature survey |

## Next Actions

1. Run full benchmark suite (all 10 experiments)
2. Generate all figures
3. Condense ARCHITECTURE.md (1624 lines) into paper-appropriate length
4. Literature survey of P2P learning systems
5. Write end-to-end learning demo
