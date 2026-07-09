# Neuron Wire: Foundational Questions & Answers

> Answers derived from codebase evidence, architecture decisions, and benchmark results.
> Date: 2026-07-09 | Commit: `94f2d39`

---

## How to Read This Document

**Audience:** Researchers and engineers evaluating the project.

**Purpose:** 20 categories of questions that every research project should be able to answer — from "what problem does this solve?" to "what would prove you wrong?"

**Not:** A technical specification. For architecture: [`ARCHITECTURE.md`](ARCHITECTURE.md). For benchmarks: [`results/`](results/). For roadmap: [`ROADMAP.md`](ROADMAP.md).

**Tone:** This document distinguishes what the project has demonstrated from what remains untested. Documenting limitations builds trust — pretending they don't exist does not.

**Glossary:**

| Term | Definition |
|------|-----------|
| **DHT (Distributed Hash Table)** | A phonebook that no single person controls — everyone holds a few pages, together they can look up any name (BitTorrent, Bitcoin) |
| **Kademlia** | DHT routing algorithm using XOR distance for bucket placement and lookups |
| **Node** | One participating device running the software |
| **UDP** | Fast but unreliable packet delivery — no delivery guarantee |
| **STDP (Spike-Timing-Dependent Plasticity)** | Biological learning rule: synaptic weight depends on precise spike timing |
| **Hebbian learning** | "Neurons that fire together, wire together" |
| **k-bucket** | Kademlia routing table slot holding up to K known peers per distance range |
| **All-Reduce** | Standard multi-GPU training — all devices exchange gradients in a ring |
| **Parameter server** | Central coordinator holding the "master copy" of a model |
| **Federated learning** | Local training on each device; weight updates sent to central server |
| **Zero-copy** | Reading data directly from receive buffer without intermediate parsing |

---

## 1. Vision & Motivation

### What is this project in one sentence?

A decentralized runtime where nodes discover peers via a Kademlia DHT, exchange neural activations over custom UDP transport, and adapt locally via Hebbian STDP — all without central coordination.

### What problem does it solve?

Distributed learning today requires either a central coordinator (parameter server, federated averaging) or a static communication topology (All-Reduce). Both assume stable connectivity and homogeneous hardware.

This project investigates whether a learning substrate can operate across arbitrary P2P topologies with node churn, no central point of failure, and an adaptive graph structure.

### Why does this problem matter?

Most distributed ML systems treat the network as an implementation detail for exchanging gradients. If intelligence emerges from networked computation, network topology becomes part of the computation itself.

### Why hasn't someone solved it already?

Networking, distributed systems, and machine learning are typically studied independently. A network-first learning runtime requires combining all three simultaneously over unreliable Internet conditions rather than tightly coupled GPU clusters.

---

## 2. Elevator Pitch

| Audience | Message |
|----------|---------|
| **30 seconds** | P2P runtime where every node maintains a small neural graph, discovers peers via DHT, exchanges activations over UDP, and adapts without a coordinator |
| **Researcher** | Kademlia-over-UDP distributed runtime with embedded Hebbian learning, adaptive graph topology, sparse gossip, and reproducible benchmarking for decentralized learning under unreliable network conditions |
| **Engineer** | Single-threaded non-blocking engine loop integrating DHT routing, reliable UDP transport, STDP learning, neurogenesis, and apoptosis — 400 KHz–1 MHz tick rate on commodity hardware |
| **General** | A decentralized compute fabric where devices collaborate without cloud coordination |

---

## 3. Problem Definition

The project investigates how decentralized learning runtimes behave under realistic network conditions — real packet loss, variable latency, churn, and no central coordinator.

Four research areas combined:
1. **Networking** — packet delivery over UDP
2. **Distributed systems** — peer discovery, fault detection, consistency
3. **Machine learning** — Hebbian STDP adaptation
4. **Runtime architecture** — single-threaded engine loop integrating all subsystems

---

## 4. Novelty

**What is genuinely new (even if preliminary):**

- **Observation about maintenance pings** — under simulated stable conditions, pinging peers for maintenance did not measurably improve routing quality. The bandwidth spent was pure overhead. This contradicts conventional DHT design wisdom.
- **Mutation-weighted gossip selection** — nodes with higher prediction error are gossiped to more frequently.
- **Unified routing and learning runtime** — same tick loop handles DHT routing and STDP learning on shared data structures.

**What builds on prior work:**

- Kademlia DHT (2002) — routing algorithm
- Hebbian learning / STDP (1949/1996) — learning rule
- FlatBuffers (2014) — serialization
- Neuroplasticity concepts — birth and death of neurons

The contribution is the integration and experimental evaluation of these ideas in a single system.

---

## 5. Architecture

Design decisions and rationale:

| Decision | Rationale |
|----------|-----------|
| Single-threaded engine loop | Simpler reasoning, no message-passing overhead between subsystems |
| Modular subsystems | DHT, learning, transport are independent modules with their own tests |
| UDP transport | Avoids TCP head-of-line blocking; system chooses which packets need reliability |
| Kademlia routing | XOR-distance lookup, no central registry |
| Sparse gossip | One peer per tick, selected by mutation weight |
| Rust implementation | Memory safety without GC, zero-cost abstractions |

The architecture favors fault tolerance and decentralization over maximum throughput. NWP will not beat NCCL All-Reduce on a homogeneous GPU cluster — that is not the goal.

---

## 6. Learning

- **Hebbian STDP updates** — synaptic weights adjust based on spike timing
- **Prediction-error-driven adaptation** — error between predicted and observed activations drives weight changes
- **Adaptive graph expansion** — prediction error exceeding threshold triggers neuron birth (neurogenesis)
- **Pruning** — inactive neurons and synapses removed (apoptosis)

There is no explicit memory buffer. Knowledge is the graph structure itself.

---

## 7. Distributed Systems

| Property | Mechanism |
|----------|-----------|
| Routing | Kademlia DHT, O(log N) lookups, 256-bit address space |
| Peer discovery | Bootstrap via seed addresses, iterative FIND_NODE |
| Fault detection | Stale timeout (configurable, default 300 s) |
| Partition recovery | Re-discovery via periodic maintenance |
| Reliable messaging | Selective retransmission over UDP, 3 tiers |
| Consistency | Eventual — no global agreement required per tick |

Intentionally avoids centralized consensus (Raft, Paxos). These are designed for consistent state across known participants, not dynamic P2P topologies.

---

## 8. Security

Current prototype limitations:

- No authentication (anyone can generate a NodeId and join)
- No encryption (wire format is plaintext)
- No replay protection
- No Sybil resistance
- No rate limiting

The adversary module detects attacks but does not defend against them. Security is future work.

---

## 9. Performance

### v3 Simulator (in-process message passing)

| Nodes | Converge | CT (sim‑s) | Avg Peers | BW | Wall Time |
|-------|----------|------------|-----------|-----|-----------|
| 100 | 100% | 0.0 | 48.7 | 96 Kbps | 0.01 s |
| 1 K | 100% | 1.0 | 56.7 | 1.1 Mbps | 0.12 s |
| 10 K | 100% | 1.0 | 57.5 | 11 Mbps | 1.54 s |
| 50 K | 100% | 3.0 | 58.6 | 71 Mbps | 13.78 s |
| 100 K | 100% | 7.0 | 59.3 | 202 Mbps | 43.43 s |

### v4 Hybrid Model (200 K active + virtual)

| Scale | Converge | CT (sim‑s) | Avg Peers | BW | Wall Time |
|-------|----------|------------|-----------|-----|-----------|
| 100 K | 99.9% | 7.5 | 116.5 | 172 Mbps | 4.9 s |
| 1 M | 99.8% | 10.0 | 147.8 | 149 Mbps | 12.4 s |
| 10 M | 98.9% | 10.0 | 142.2 | 137 Mbps | 12.2 s |
| 100 M | 99.8% | 12.5 | 176.2 | 125 Mbps | 15.5 s |
| **1 B** | **99.5%** | **12.5** | **176.2** | **125 Mbps** | **15.6 s** |

### Key findings

- Avg peers saturates (~59 for v3, ~176 for v4 with MAX_PEERS=500)
- Convergence time grows as O(log N): 7.5 s → 12.5 s from 100 K to 1 B
- Bandwidth per node is approximately constant (~2.3 Kbps/node)
- Hybrid model makes simulation cost independent of N for N > active_max

**Not yet measured:**
- WAN latency effects (all benchmarks are localhost)
- CPU usage per tick
- RAM under high graph expansion
- Multi-core scalability

---

## 10. Mathematical Basis

See [`FORMAL_MODEL.md`](FORMAL_MODEL.md) (1,760 lines, 17 sections) for full derivations.

**Known:**
- Routing complexity: O(log N) Kademlia lookups
- Space complexity: O(K × B) where K=20 (bucket capacity) and B=256 (address bits)
- DHT convergence theorem: probability of node isolation → 0 as K → log N

**Open:**
- No formal convergence proof for learning dynamics
- No closed-form bandwidth model as a function of N
- No analytical scaling ceiling

---

## 11. Benchmarks

Current benchmarks measure:
- DHT convergence time across 5 scales (100–100 K v3, 100 K–1 B v4)
- Bandwidth consumption vs. node count
- Maintenance mode comparison (fixed vs. SGA)
- Fault tolerance (adversary injection)

**Why believe them:** All experiments are deterministic (fixed seed = identical results). Raw CSV output is checked into the repository. CI validates against known-good reference values.

**Limitation:** All benchmarks run on localhost. Real Internet conditions may produce different results.

---

## 12. Failure Modes

| Mode | Status |
|------|--------|
| Packet loss up to 10% | Transport degrades gracefully |
| Node crash | Handled in simulation |
| Network partition | Handled in simulation |
| Replay attack | Detected but not defended |
| **Real WAN conditions** | **Not tested** |
| **High churn (>10%/s)** | **Not tested** |
| **Memory pressure** | **Not tested** |

---

## 13. Evidence

**Confirmed (tested and reproducible):**
- DHT converges across all tested scales (100 → 1 B)
- SGA uses 1.9–2.45× more bandwidth than fixed maintenance at ≤300 s
- Under tested conditions, maintenance pings do not measurably improve routing quality
- STDP weight updates, apoptosis, zero-copy serialization all verified by unit tests

**Hypotheses (untested):**
- WAN deployment works at all
- System converges under high node churn
- Learning produces useful results over real Internet RTT
- System scales beyond 50 K nodes without redesign

---

## 14. Research Methodology

| Component | Detail |
|-----------|--------|
| **Hypothesis** | Sparse Gradient Aging reduces DHT maintenance bandwidth |
| **Null hypothesis** | SGA produces same or greater bandwidth than fixed-interval maintenance |
| **Result** | Null hypothesis supported — SGA increased bandwidth 1.9–2.45× |
| **IVs** | Maintenance mode (fixed vs. SGA), node count (10, 25, 50) |
| **DVs** | Bandwidth, packet count, convergence time, max/avg peers |
| **Controls** | Seed (42), duration (30 s), gossip interval (500 ticks), tick interval (1 ms) |

**Limitation:** Current benchmarks use 1 trial per configuration (deterministic = zero variance, zero statistical power). Multi-trial runs with confidence intervals are future work.

---

## 15. Comparison

| Aspect | Centralized (NCCL) | Neuron Wire |
|--------|--------------------|-------------|
| Coordination | Central coordinator | None |
| Topology | Fixed (ring/tree) | Adaptive |
| Hardware | Homogeneous GPUs | Heterogeneous devices |
| Fault tolerance | Coordinator fail = system fail | No single point of failure |
| Network req. | µs latency, zero loss | Tolerates loss and latency |
| Convergence speed | Fast | Slower |

**Not yet compared:** Against federated learning, Ray, BitTensor, Gensyn. A proper comparison is on the roadmap.

---

## 16. Reproducibility

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release
cargo run --example simulate -- --paper-mode --nodes 3 --duration 10
```

All experiment CSVs are in `results/`. One-command reproduction via `scripts/reproduce.sh`. Full CI validates against known-good reference values.

---

## 17. Engineering

| Metric | Value |
|--------|-------|
| Language | Rust 2021 edition |
| Source files | 55 Rust, ~8,000 LOC |
| Tests | 10 `#[test]` annotations + integration + property-based |
| CI | GitHub Actions: 4 workflows (build+test, docs, release, reproduction validation) |
| Build | `opt-level="z"`, LTO, stripped — single statically linked binary |
| Dependencies | 5 direct (crc32fast, rand, serde, toml, csv) |
| Memory safety | No `unsafe` outside zero-copy module |
| Fuzz testing | cargo-fuzz target for header parsing |
| Engine tick rate | ~400 KHz–1 MHz on commodity x86-64 |

---

## 18. Open Research Questions

See [`RESEARCH_QUESTIONS.md`](RESEARCH_QUESTIONS.md) for the full list of 8 scientific questions.

**Critical open question:** Deploy 100 nodes across 3 continents. If the DHT fails to converge within 30 seconds under real Internet conditions, the architecture does not work outside localhost.

---

## 19. References

1. Maymounkov, P., & Mazières, D. (2002). Kademlia: A peer-to-peer information system based on the XOR metric. *IPTPS*.
2. Li, M., et al. (2014). Scaling distributed machine learning with the parameter server. *OSDI*.
3. Dean, J., et al. (2012). Large scale distributed deep networks. *NIPS*.
4. Hebb, D. O. (1949). *The Organization of Behavior*. Wiley & Sons.
5. Gerstner, W., et al. (1996). A neuronal learning rule for sub-millisecond temporal coding. *Nature*.
6. Sergeev, A., & Del Balso, M. (2018). Horovod: fast and easy distributed deep learning in TensorFlow. *arXiv:1802.05799*.
7. Bonawitz, K., et al. (2019). Towards federated learning at scale: System design. *MLSys*.
8. Stoica, I., et al. (2017). Ray: A distributed framework for emerging AI applications. *OSDI*.
