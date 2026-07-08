# Neuron-Wire: Foundational Questions & Answers

> Answered from codebase evidence, architecture decisions, and benchmark results.
> Date: 2026-06-26 | Commit: `d0875d9`

---

## How to Read This Document

**Who wrote this:** The Neuron Wire team at Zylvon — building open infrastructure for decentralized AI.

**Who this is for:** Researchers, engineers, and anyone who wants to understand the project without reading 8,000 lines of code.

**What this document is:** 20 categories of questions that every research project should be able to answer — from "what problem does this solve?" to "what would prove you wrong?"

**What this document is not:** A technical specification. For architecture details, see `ARCHITECTURE.md`. For benchmark data, see `results/`. For the future plan, see `ROADMAP.md`.

**A note on the language:** This document tries to be honest about what the project has actually demonstrated versus what remains untested. If something sounds like a limitation, it's intentional — documenting what you don't know builds more trust than pretending you do.

**Key terms explained (for readers with no CS background):**

| Term | Plain English |
|------|---------------|
| **DHT (Distributed Hash Table)** | A phonebook that no single person controls — everyone holds a few pages, and together they can look up any name. Used by BitTorrent and Bitcoin. |
| **Kademlia** | A specific way of organizing that phonebook (the DHT) so lookups are fast — like organizing contacts by how far apart their phone numbers are in number-space, not alphabetically. |
| **Node** | One participating device (laptop, phone, server) running the software. |
| **Peer-to-peer (P2P)** | Devices talk directly to each other instead of going through a central server. Like a group chat instead of a newsletter. |
| **UDP** | A way of sending packets over the Internet that's fast but unreliable — like shouting across a room. No guarantee the other person heard you. |
| **TCP** | The alternative to UDP — reliable but slower, like registered mail with delivery confirmation. |
| **STDP (Spike-Timing-Dependent Plasticity)** | A rule for how connections between simulated neurons get stronger or weaker based on which one fires first. "Cells that fire together, wire together" — but with precise timing. |
| **Hebbian learning** | "Neurons that fire together, wire together." The simplest explanation for how learning happens in biological brains. |
| **All-Reduce** | The standard way to train AI across many GPUs — every device sends its results to everyone else in a ring, and they all average together. Requires fast, reliable connections. |
| **Parameter server** | A central coordinator that holds the "master copy" of a model and sends updates to worker devices. Single point of failure. |
| **Federated learning** | Training across phones/laptops where each device learns locally and only sends weight updates to a central server (instead of raw data). Still needs that server. |
| **k-bucket** | A Kademlia routing table slot that holds up to 8 known peers. Like a contact list with room for 8 entries per "distance range." |
| **SGA (Sparse Gradient Aging)** | A strategy for deciding which peers to ping for maintenance — pings older (staler) peers more often. The project's main experiment tested this vs. fixed-interval pinging. |
| **CRC (Cyclic Redundancy Check)** | A mathematical fingerprint on each packet. If the packet gets corrupted in transit, the fingerprint won't match and the receiver knows to discard it. |
| **Zero-copy** | Reading data directly from the receive buffer without copying or reformatting it — like reading a book without having to rewrite each page first. Fast. |

---

## 1. Vision & Motivation

### What is this project in one sentence?

A decentralized runtime where nodes discover each other via a DHT (a distributed phonebook), exchange neural activations through a custom UDP transport (fast but unreliable messaging), and learn locally via Hebbian STDP (a biological learning rule), all without central coordination.

### What problem does it solve?

Distributed learning today typically requires either a central coordinator (parameter server, federated averaging) or a static communication topology (All-Reduce). Both approaches assume stable connectivity and relatively homogeneous hardware.

This project investigates whether a learning substrate can operate across arbitrary peer-to-peer topologies with node churn (devices joining and leaving constantly), no central point of failure, and an adaptive graph structure (the network changes shape as nodes come and go).

### Why does this problem matter?

If intelligence emerges from networked computation, then network topology becomes part of the computation itself rather than merely the transport layer.

Most distributed machine learning systems treat the network as an implementation detail used to exchange gradients (numeric values that tell the model how to improve). Neuron-Wire instead explores whether the network itself can become the computational substrate — meaning the connections between devices are as important as what each device computes.

### Who experiences this problem today?

* Researchers training across heterogeneous devices (different hardware, different speeds)
* Edge-computing systems (processing data where it's collected, not in a data center)
* Mobile and IoT learning deployments (phones, sensors, smart devices)
* Researchers exploring decentralized AI (P2P intelligence without Big Tech control)
* Anyone wanting collaborative learning without centralized infrastructure

### Why hasn't someone solved it already?

Most research separates networking, distributed systems, and machine learning into independent problems. Networking people don't usually design learning algorithms, and ML people don't usually design P2P protocols.

A network-first learning runtime requires combining all three simultaneously while operating over unreliable Internet conditions rather than tightly coupled GPU clusters.

### What inspired this architecture?

* **Kademlia DHT** — the same routing algorithm that BitTorrent uses, adapted for node discovery
* **FlatBuffers** — a Google serialization library that enables zero-copy reads (no parsing step)
* **Hebbian learning** — the foundational theory of how biological brains learn
* **Spike-Timing-Dependent Plasticity (STDP)** — a precise, time-based version of Hebbian learning
* **Neuroplasticity** — biological concepts like neuron birth (neurogenesis) and death (apoptosis), where the brain physically rewires itself based on experience

### If this project disappeared tomorrow, who would notice?

Nobody yet.

It is currently a research prototype. That's a limitation, not a secret — and part of the reason this document lists what isn't done alongside what is.

### Why should anyone care?

The current simulator suggests that, under the tested conditions, maintenance pings (periodic "are you alive?" messages between nodes) did not measurably improve routing quality once routing tables were saturated. This is counterintuitive — conventional wisdom says you need those pings.

Whether this observation generalizes beyond the simulator (to real Internet conditions with real packet loss and latency) is an open research question.

### Why should anyone trust the work?

* Passing automated test suite (72 tests)
* Reproducible benchmarks (anyone can run the same command and get the same numbers)
* Raw experimental data checked into the repository (not just summary statistics)
* Public source code
* Deterministic simulation mode (same seed = same results every time)

### Why does the world need another distributed runtime?

The objective is not to create another runtime.

The objective is to generate evidence about decentralized learning under realistic network constraints — evidence that currently doesn't exist in published form.

### What concrete capability is the runtime trying to unlock?

Zero-infrastructure collaborative learning.

Ordinary devices — a phone in Nepal, a laptop in Germany, a Raspberry Pi in Brazil — should be able to discover each other, form a learning network, and collaboratively improve without requiring centralized servers, manual configuration, or fixed infrastructure.

Whether this is practical over real Internet conditions is the central research question. Everything else in this project exists to help answer that question.

---

## 2. Elevator Pitch

### 30 Seconds

A peer-to-peer runtime where every node maintains a small neural graph, discovers peers through a distributed hash table (a shared phonebook no one controls), exchanges activation information over a custom UDP protocol (fast, unreliable messaging), and continuously adapts without any centralized coordinator.

### To a Professor

A Kademlia-over-UDP distributed runtime with embedded Hebbian learning, adaptive graph topology, sparse gossip, and reproducible benchmarking designed to study decentralized learning under unreliable network conditions.

### To a CEO

A decentralized compute fabric that allows heterogeneous devices (phones, laptops, edge servers) to collaborate without relying on cloud coordination.

### To a High School Student

Imagine hundreds of phones teaching one another by exchanging tiny pieces of information instead of sending everything to one giant server. If a phone leaves, the rest adapt. No one is in charge.

### To Your Grandmother

It's like a group project with no leader. Everyone shares what they know with nearby people until the whole group improves together. If someone leaves, the rest keep going.

### Without mentioning AI

A decentralized communication runtime where independent devices exchange structured information, automatically discover peers, recover from failures, and converge on a shared state without centralized coordination.

---

## 3. Problem Definition

The project investigates how decentralized learning runtimes behave under realistic network conditions — meaning real packet loss, variable latency, nodes joining and leaving, and no central coordinator to fall back on.

It combines four research areas:
* **Networking** — how packets get from one node to another over UDP
* **Distributed systems** — how nodes find each other, detect failures, and stay consistent
* **Machine learning** — how nodes learn from data using Hebbian STDP rules
* **Runtime architecture** — how the engine loop schedules all these subsystems in a single thread

A centralized parameter server would solve the engineering problem more simply. You could just install NCCL and All-Reduce and be done.

However, it would not answer the research question of whether learning can emerge without centralized coordination. That question requires building something that doesn't exist yet, not using something that already does.

---

## 4. Novelty

**What is genuinely new (even if preliminary):**

* **Experimental observation about maintenance pings** — under simulated stable conditions, pinging peers for maintenance didn't improve routing quality. The bandwidth spent on those pings was pure overhead. This contradicts conventional DHT design wisdom.
* **Mutation-weighted gossip selection** — nodes with higher prediction error get gossiped to more frequently, biasing communication toward "interesting" peers.
* **Adaptive graph expansion and pruning** — the neural graph grows when the system encounters novel observations and shrinks when neurons stop contributing.
* **Unified routing and learning runtime** — the same tick loop handles both DHT routing and STDP learning on shared data structures.

**What builds on prior work (not claiming originality):**

* Kademlia DHT (2002) — the routing algorithm
* Hebbian learning / STDP (1949/1996) — the learning rule
* FlatBuffers (2014) — the serialization format
* Apoptosis in neural networks — pruning unused connections

The contribution is not inventing these ideas individually but integrating them into a single system and experimentally evaluating the result.

**If you removed the novel parts:** A competent but unremarkable Kademlia DHT with UDP transport and 72 tests.

---

## 5. Architecture

Major design decisions and why:

* **Single asynchronous engine loop** — not actors or microservices. Simpler, easier to reason about, no message-passing overhead between subsystems.
* **Modular subsystems** — DHT, learning, transport are independent modules with their own tests. Can be swapped or removed.
* **UDP transport** — not TCP. TCP's head-of-line blocking means one lost packet stalls the entire queue. UDP lets the system choose which packets need reliability.
* **Kademlia routing** — not a centralized registry. Nodes find each other through XOR-distance-based lookup, same algorithm as BitTorrent.
* **Sparse gossip** — not full mesh. Each tick, a node gossips with only one peer, selected by mutation weight.
* **Rust implementation** — not Go or C++. Memory safety without garbage collection, zero-cost abstractions, and the type system prevents whole categories of bugs.

The architecture favors fault tolerance and decentralization over maximum throughput. This is a deliberate trade-off: NWP will never beat NCCL All-Reduce on a homogeneous GPU cluster, but that's not the goal.

---

## 6. Learning

Learning consists of:
* **Hebbian STDP updates** — synaptic weights (connection strengths between simulated neurons) adjust based on which neuron fires first
* **Prediction-error-driven adaptation** — when the system predicts an activation and gets it wrong, the error drives weight changes
* **Adaptive graph expansion** — when prediction error exceeds a threshold, a new neuron is spawned (neurogenesis)
* **Pruning of inactive structures** — neurons that consistently fail to contribute to prediction accuracy get removed (apoptosis)

Knowledge is represented as weighted edges within a sparse graph. There is no explicit "memory buffer" — memory is the graph structure itself.

---

## 7. Distributed Systems

The runtime provides:
* **Kademlia routing** — O(log N) lookups in a 160-bit address space
* **Peer discovery** — bootstrap via seed addresses, then iterative FIND_NODE
* **Fault detection** — stale timeout (configurable, default 300 seconds)
* **Partition recovery** — when a network split heals, nodes re-discover via periodic maintenance
* **Reliable messaging over UDP** — selective retransmission for packets that need it, best-effort for the rest
* **Eventual consistency** — nodes don't need to agree at every tick, only converge over time

It intentionally avoids centralized consensus mechanisms like Raft or Paxos. Those are designed for consistent state across known participants, not for dynamic P2P topologies with unknown churn.

---

## 8. Security

Current prototype limitations — stated directly:

* No authentication (anyone can generate a NodeId and join)
* No encryption (wire format is plain FlatBuffer)
* No replay protection (packets can be captured and resent)
* No Sybil resistance (one entity can pretend to be many nodes)
* No rate limiting (a malicious node can flood the network)

The adversary module in the codebase *detects* attacks but doesn't *defend* against them. Security is future work, not a current contribution.

This is acceptable for a research prototype focused on routing and learning dynamics. A production system would need all of these.

---

## 9. Performance

**Measured (in simulator, on localhost, 30-second runs):**

| Metric | 10 nodes | 50 nodes |
|--------|----------|----------|
| Bandwidth (fixed mode) | 603 Kbps | 9,525 Kbps |
| Bandwidth (SGA mode) | 1,146 Kbps | 20,325 Kbps |
| Packets/sec (SGA) | ~1,303 | ~21,994 |
| Routing convergence | 3.0s | 3.0s |

**Not yet measured:**
* CPU usage per tick
* RAM per node (estimated ~few hundred KB)
* WAN latency effects
* Memory under high graph expansion
* Parallelization scalability

These are on the roadmap.

---

## 10. Mathematical Questions

**What we know:**
* **Routing complexity:** O(log N) Kademlia lookups in theory (not yet measured against N)
* **Space complexity:** O(K × B) where K=8 (bucket capacity) and B=160 (address bits) = at most 1,280 peer entries per node
* **Communication complexity:** O(N²) during bootstrap (all nodes discover all others), then O(N × gossip_interval) in steady state

**What we don't know:**
* No formal convergence proof for the learning dynamics
* No closed-form bandwidth model as a function of N
* No scaling ceiling derived analytically

Empirical measurements fill some of these gaps (DHT converges in ≈3s across all tested sizes), but formal analysis is future work.

---

## 11. Benchmarks

Current benchmarks measure:
* Routing convergence time
* Bandwidth consumption
* Packet counts
* Fault tolerance (adversary injection)
* Scalability (10, 25, 50 nodes)

**Why these benchmarks:** To answer the question "what is the cost of decentralized routing vs. centralized coordination?"

**Why should you believe them:** All experiments are deterministic (fixed seed = identical results every time). Raw CSV output is checked into the repository alongside summary statistics. The CI pipeline validates every run against known-good reference values.

**Limitation:** Current benchmarks compare two modes of the same system (fixed vs. SGA maintenance). No baseline comparisons against external systems (parameter server, All-Reduce, etc.) have been performed.

---

## 12. Failure Modes

**What breaks first:** The transport layer under high packet loss. The reliable queue retransmits up to 3 times, but above ~10% loss, gradient exchange degrades.

**What hasn't been tested at all:** Real WAN conditions, high churn rates (>10% nodes/second), memory pressure, CPU starvation.

**What we know from adversary tests:** The system handles node crashes, network partitions, corrupted packets, replay attacks, and malicious routing responses — in simulation.

**Critical limitation that affects all failure mode claims:** Everything is tested on localhost UDP. Real networks have NATs, firewalls, asymmetric routing, and variable latency that the simulator doesn't model.

Documenting these limitations is considered part of the research contribution — it tells the next researcher where to focus.

---

## 13. Evidence

**Confirmed (supported by tests and benchmarks):**
* DHT converges in ≈3s across 10–50 nodes
* SGA uses 1.9–2.45× more bandwidth than fixed maintenance at ≤300s run duration
* Under tested conditions, maintenance pings don't measurably improve routing quality
* STDP weight updates work correctly (unit tests pass)
* Apoptosis correctly detects and prunes inactive structures
* Zero-copy serialization roundtrips correctly
* Runtime maintains 1000 ticks/sec on localhost

**Untested (hypotheses, not evidence):**
* WAN deployment works at all
* System converges under high node churn
* Learning produces useful results over real Internet RTT
* System scales beyond 50 nodes without redesign

Distinguishing these two categories is one of the most important things this document does.

---

## 14. Research Methodology

The project treats itself as an experimental investigation, not an engineering showcase:

* **Hypothesis:** Sparse Gradient Aging reduces DHT maintenance bandwidth
* **Null hypothesis:** SGA produces the same or greater bandwidth as fixed-interval maintenance (this is what the evidence supported — the hypothesis was rejected)
* **Independent variables:** Maintenance mode (fixed vs. SGA), node count (10, 25, 50)
* **Dependent variables:** Bandwidth, packet count, convergence time, max/avg peers
* **Controlled variables:** Seed (42), duration (30s), gossip interval (500 ticks), tick interval (1ms), topology (local mesh)

**Honest limitation:** The benchmarks ran 1 trial per configuration (6 total). With deterministic simulation, variance is zero, but statistical power is also zero. Multi-trial runs with confidence intervals are future work.

---

## 15. Comparison

Neuron-Wire is intended to complement rather than replace existing systems.

**Where NWP makes different trade-offs:**

| Aspect | Centralized (e.g., NCCL All-Reduce) | Neuron-Wire |
|--------|--------------------------------------|-------------|
| Coordination | Central coordinator | None |
| Topology | Fixed (ring/tree) | Adaptive |
| Hardware assumption | Homogeneous GPUs | Heterogeneous devices |
| Fault tolerance | Coordinator fail = system fail | No single point of failure |
| Network requirement | µs latency, zero loss | Tolerates loss and latency |
| Convergence speed | Fast | Slower |

**Where no comparison exists yet:** Against federated learning (FL requires a server but tolerates heterogeneous devices), against Ray (distributed task execution, not learning), against Bittensor/Gensyn (decentralized AI projects — not benchmarked because they aren't open research artifacts with reproducible results).

A proper comparison is on the roadmap. For now, the honest answer is "not yet benchmarked."

---

## 16. Reproducibility

**Can you build it in under 10 minutes?** Yes, if Rust is installed:
```
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release
cargo run --example simulate -- --paper-mode --nodes 3 --duration 10
```

**Can you verify every published number?** Yes — all 6 experiment directories are in `results/` with raw CSV. Anyone can run the same command with the same seed and get the same numbers.

**Can you reproduce every figure?** Not yet — the reproducibility infrastructure (experiment configs, figure generation scripts) doesn't exist. That's the highest-priority infrastructure gap.

---

## 17. Engineering

| Metric | Value |
|--------|-------|
| Language | Rust |
| Lines of code | ~8,000 |
| Source files | 26 |
| Public modules | 14 |
| Tests | 72 (all passing) |
| Test coverage | Not measured |
| CI | GitHub Actions (build + test + clippy + benchmark validation) |
| Documentation | `ARCHITECTURE.md` (1,621 lines), auto-generated `cargo doc` |
| Memory safety | No `unsafe` outside the zero-copy module |
| Fuzz testing | None |
| Performance profiling | None |

---

## 18. Open Source

**Why should someone contribute?** To help answer a real research question — can P2P collaborative learning work over real Internet conditions? The project needs help with networking, security, benchmarking, and real-world deployments.

**Why should someone deploy it?** They shouldn't (yet). It's a research prototype, not production infrastructure. The roadmap explicitly includes the testbed phase before any deployment recommendation.

**Why should someone trust it?** 72 tests pass. The surprising finding is reproducible. The codebase is small enough to audit (~8,000 lines). The limitations are documented, not hidden.

---

## 19. Admissions Officer Questions

This project demonstrates, through the work itself rather than through self-promotion:

* **Independent research** — identifying a question where the answer isn't known
* **Systems engineering** — designing and building a modular distributed runtime in Rust
* **Scientific methodology** — formulating hypotheses, running controlled experiments, distinguishing evidence from assumptions
* **Willingness to publish unexpected results** — the SGA hypothesis failed, and that failure is the central finding
* **Documentation discipline** — maintaining a research journal (Obsidian vault), architecture doc, benchmark reports, and this Q&A

Rather than proving a predetermined conclusion, the project investigates an open research question through reproducible experiments. The question remains open, and that's the point.

---

## 20. The Killer Questions

**What assumptions does your entire architecture depend on?**
* UDP connectivity (no HTTP-only environments)
* Peer reachability (nodes can talk to each other, at least via NAT traversal)
* Approximate clock synchronization (seconds, not milliseconds precision)
* Successful DHT convergence (at least one seed node reachable at bootstrap)

**What experiment could prove you wrong?**
Deploy 100 nodes across 3 continents. If the DHT fails to converge within 30 seconds under real Internet conditions, the architecture doesn't work outside localhost.

**What result would make you abandon the current approach?**
If bandwidth scales super-linearly with node count on the WAN testbed (suggesting an O(N²) bottleneck), the DHT design needs replacement with a hierarchical or clustered topology.

**If the central idea fails, what is still valuable?**
The reproducibility framework, benchmark methodology, engineering design, maintenance-ping finding, and the documented admission that a designed approach was tested and falsified.

**The strongest criticism of this work right now:**

> The implementation has only been validated in simulation, lacks a production security model, has limited statistical evaluation, and has not yet been benchmarked against major distributed learning systems.

The project embraces this criticism by treating it as the roadmap rather than attempting to hide it.

**What is the long-term vision?**

The long-term objective is to determine whether decentralized collaborative learning can become practical without centralized infrastructure.

If successful, ordinary devices anywhere in the world — a phone, a laptop, a Raspberry Pi — could automatically discover one another, exchange knowledge, adapt to failures, and collaboratively learn without requiring any centralized coordination.

Whether this vision can survive real Internet conditions remains the defining research question that motivates Neuron-Wire.

---

## References

1. Maymounkov, P., & Mazières, D. (2002). Kademlia: A peer-to-peer information system based on the XOR metric. *IPTPS*.
2. Li, M., et al. (2014). Scaling distributed machine learning with the parameter server. *OSDI*.
3. Dean, J., et al. (2012). Large scale distributed deep networks. *NIPS*.
4. Hebb, D. O. (1949). *The Organization of Behavior*. Wiley & Sons.
5. Gerstner, W., et al. (1996). A neuronal learning rule for sub-millisecond temporal coding. *Nature*.
6. Google FlatBuffers. (2014). https://flatbuffers.dev — Zero-copy serialization library.
7. Sergeev, A., & Del Balso, M. (2018). Horovod: fast and easy distributed deep learning in TensorFlow. *arXiv:1802.05799*.
8. Bonawitz, K., et al. (2019). Towards federated learning at scale: System design. *MLSys*.
9. Stoica, I., et al. (2017). Ray: A distributed framework for emerging AI applications. *OSDI*.
10. Castro, M., et al. (2020). One size does not fit all: The case for federated learning over heterogeneous networks. *arXiv:2006.12291*.
