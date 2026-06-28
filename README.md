# Neuron Wire Protocol (NWP)

**A Decentralised Adaptive Runtime for Large-Scale Distributed Learning**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![Tests](https://img.shields.io/badge/tests-77%2F77-brightgreen)](#)
[![Lines of Rust](https://img.shields.io/badge/rust-7%2C975%20LOC-blue)](#)
[![Crates.io](https://img.shields.io/badge/crate-v0.2.0-blue)](#)
[![Live Dashboard](https://img.shields.io/badge/dashboard-live-00c8ff)](https://neuron-wire-dashboard.vercel.app)
[![Reproducible](https://img.shields.io/badge/reproducible-yes-brightgreen)](REPRODUCIBILITY.md)
[![Architecture](https://img.shields.io/badge/architecture-formal-7c3aed)](ARCHITECTURE.md)
[![Q&A](https://img.shields.io/badge/q%26a-foundational-ff6b6b)](FOUNDATIONAL_QNA.md)
[![Roadmap](https://img.shields.io/badge/roadmap-20%20directions-ffc107)](ROADMAP.md)
[![Lessons](https://img.shields.io/badge/lessons-learned-00c853)](LESSONS_LEARNED.md)

---

## Table of Contents

- [Problem Statement](#problem-statement)
- [Why Existing Approaches Struggle](#why-existing-approaches-struggle)
- [The Neural Wire Protocol](#the-neural-wire-protocol)
- [Architecture Overview](#architecture-overview)
  - [Subsystem Map](#subsystem-map)
  - [The 6-Phase Engine Loop](#the-6-phase-engine-loop)
  - [DHT: Latency-Weighted Kademlia](#dht-latency-weighted-kademlia)
  - [UDP Transport with Gradient Decay](#udp-transport-with-gradient-decay)
  - [Neural Computation: Forward Pass + Hebbian STDP](#neural-computation-forward-pass--hebbian-stdp)
  - [Neurogenesis: Surprise-Driven Neuron Birth](#neurogenesis-surprise-driven-neuron-birth)
  - [Apoptosis: Programmed Neuron Death](#apoptosis-programmed-neuron-death)
- [Key Numbers](#key-numbers)
- [Reproducibility](#reproducibility)
- [Limitations](#limitations)
- [Documentation](#documentation)
- [Roadmap](#roadmap)
- [License](#license)

---

## Problem Statement

Modern machine learning trains on centralised GPU clusters. This approach has three hard ceilings:

1. **Data gravity** — training data must move to the compute, not the other way around. Privacy regulations (GDPR, CCPA) and bandwidth economics make this increasingly untenable.
2. **Parameter-server bottlenecks** — every gradient update flows through a central coordinator. At 10⁹ parameters, synchronisation latency dominates wall-clock time even with All-Reduce.
3. **Static topology** — once a model architecture is fixed, it cannot grow, shrink, or rewire in response to new data sources or node churn. The network is a snapshot, not an organism.

Neuron Wire Protocol (NWP) treats these constraints as a *distributed systems* problem rather than an ML problem. Instead of centralising compute, NWP distributes both the *model* and the *learning process* across a decentralised P2P network where each node is a self-contained neural substrate that discovers peers, exchanges activation gradients, and adapts its local connectivity through biologically inspired mechanisms — all without a coordinator.

---

## Why Existing Approaches Struggle

| Approach | Limitation |
|---|---|
| **Federated Learning (FL)** | Central coordinator, synchronous rounds, fixed architecture. Clients train locally then upload weights — no peer-to-peer emergence. |
| **All-Reduce / Ring-Reduce** | Requires a static, known set of participants. Gradient synchronisation cost grows linearly with model size. No support for dynamic topology. |
| **Gossip-Based SGD** | Asynchronous, but still operates on a fixed parameter vector. No notion of individual neurons, neurogenesis, or apoptosis. |
| **Blockchain-Based ML** | Enormous overhead (consensus every transaction, storage replication). Throughput is measured in transactions/second, not activations/second. |
| **Kademlia DHT (vanilla)** | XOR routing is excellent for file lookup but treats all peers equally. No latency awareness, no gradient-weight decay, no support for soft-state updates. |

NWP is none of these. It is a purpose-built substrate for *adaptive distributed neural computation* — not a general-purpose ML framework retrofitted for distribution, but a distributed systems protocol designed from the ground up with neural computation as its first-class workload.

---

## The Neural Wire Protocol

NWP is a binary wire protocol (~40 bytes overhead per message) that defines how neural state — activations, gradients, spike events, consensus votes — is serialised, routed, decayed, and merged across a dynamic P2P network of heterogeneous nodes.

Every message is framed as:

```
[16 bytes] Transport Header   (seq, ack, bitfield, timestamp)
[16 bytes] NWP Message Header (magic, version, type, flags, body_len, CRC32)
[N bytes]  FlatBuffer Body    (zero-copy field access, no deserialisation)
```

Message types: `Ping`, `Pong`, `Command`, `Spike`, `Readiness`, `Data` (gradients), `Consensus`.

---

## Architecture Overview

### Subsystem Map

```
┌────────────────────────────────────────────────────────────────────┐
│                      EngineLoop (single thread)                     │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐   │
│  │  DHT Routing  │  │  UDP Socket  │  │  Neural Computation      │   │
│  │  (Kademlia +  │◄─┤  (recv/send) │──┤                         │   │
│  │   latency wt)  │  └──────────────┘  │  ┌─────────────────┐   │   │
│  └──────────────┘                      │  │ ForwardPass     │   │   │
│        │                               │  │ (tanh propagate)│   │   │
│        ▼                               │  └─────────────────┘   │   │
│  ┌──────────────┐                      │  ┌─────────────────┐   │   │
│  │ Neurogenesis  │◄──── surprise ──────┤  │ Hebbian (STDP)  │   │   │
│  │ (neuron birth)│                      │  └─────────────────┘   │   │
│  └──────────────┘                      └─────────────────────────┘   │
│        │                                      │                       │
│        ▼                                      ▼                       │
│  ┌──────────────┐                      ┌──────────────┐              │
│  │  Apoptosis    │◄──── prune ─────────┤  Component   │              │
│  │ (neuron death)│                      │  Store (ECS) │              │
│  └──────────────┘                      └──────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

### The 6-Phase Engine Loop

The engine is a single-threaded, non-blocking loop that owns the UDP socket. Every tick (~1 μs–1 ms, 400 KHz–1 MHz on commodity hardware) it executes six phases:

| Phase | What | Frequency |
|---|---|---|
| **1. Drain UDP** | `recv_from()` in a tight loop until `WouldBlock`. All available messages are consumed before processing begins. | Every tick |
| **2. Drain Outbound** | `try_recv()` on the `mpsc` channel. Queued packets (from DHT, Hebbian gossip, etc.) are sent over UDP. | Every tick |
| **3. Neural Compute** | ForwardPass (propagate → squash → compare → surprise) then Hebbian STDP (weight update → micro-prune → gossip). | Every tick |
| **4. Retransmit** | Scan reliable queue for expired or unacknowledged DATA/CONSENSUS frames. | Every 10 ticks (~10 ms) |
| **5. Apoptosis** | Evict dead DHT nodes, expired pings, orphaned transport frames, prune stale synapses. | Every 1000 ticks (~1 s) |
| **6. Yield** | If the tick completed faster than the target interval, the OS blocks the thread on `recv_from()` (0% CPU when idle). | Every tick |

No Tokio. No async runtime. No work-stealing scheduler. The rationale is documented in [`engine_loop.rs`](src/engine_loop.rs): a single-threaded `recv_from()` loop with a 1 ms read timeout eliminates scheduler jitter, avoids 50+ transitive crate dependencies, and sustains maximum throughput on free-tier VPS hardware (512 MB RAM, shared CPU).

### DHT: Latency-Weighted Kademlia

Standard Kademlia uses XOR distance for bucket placement and time-based eviction. NWP's hybrid uses **XOR for placement** (guarantees global reachability) and **latency for ranking and eviction** within each bucket.

```
┌─────────────────────────────────────────────┐
│             256 K-Buckets                     │
│                                               │
│  Bucket 0           Bucket 1         Bucket 255
│  ┌──────────────┐  ┌──────────────┐  ┌──────┐
│  │ node A 10ms  │  │ node D 22ms  │  │ ...  │
│  │ node B 15ms  │  │ node E 34ms  │  └──────┘
│  │ node C 55ms  │  │ node F 91ms  │
│  └──────────────┘  └──────────────┘
│       ▲                    ▲
│  sorted by latency   sorted by latency
│  (fastest first)     (fastest first)
└─────────────────────────────────────────────┘
```

- **K = 20** entries per bucket.
- **Insertion**: if the bucket is full, the highest-latency entry is evicted in favour of a faster peer.
- **Latency EMA**: `latency = latency × 0.7 + sample × 0.3`.
- **Failure tracking**: after 3 consecutive failures, the entry is removed.
- **Bootstrap priority**: (1) peer cache file, (2) DNS seeds (`_dht.seeds.<domain>`), (3) hardcoded seed addresses, (4) passive listening.
- **Node types**: General, Language, Reasoning, Memory, Vision, Audio, Consensus, Gateway — enabling cluster-aware gradient gossip (same-type peers prioritised during Hebbian exchange).

### UDP Transport with Gradient Decay

NWP implements a custom reliable transport over UDP. Each datagram carries a 16-byte transport header:

```
[ 0- 3] sequence_number  (u32, monotonic per sender)
[ 4- 7] ack_number       (u32, last contiguous seq received)
[ 8-11] ack_bitfield     (u32, bitmask of next 32 packets)
[12-15] timestamp        (u32, sender's local tick in ms)
```

**Reliability tiers:**

| Tier | Max Retries | Used For |
|---|---|---|
| BestEffort | 0 | SPIKE, COMMAND, READINESS, GOSSIP |
| Data | 3 | Gradient DATA frames |
| Consensus | 5 | CONSENSUS frames |

**Gradient weight decay** uses exponential decay:

```
weight = e^(-ln(2) × Δt / half_life)
```

- At Δt = half_life (100 ms default): weight = 0.5
- At Δt = 10 × half_life (1 s): weight ≈ 0.001 (dropped)

This means a gradient that arrives 500 ms late contributes only 3% of its original value — the system naturally prioritises fresh information without needing explicit timeouts or clock synchronisation.

### Neural Computation: Forward Pass + Hebbian STDP

**ForwardPassSystem** — the neural heartbeat. Every tick:

1. **Leak**: decay all activations by 0.95 (prevents runaway firing).
2. **Propagate**: for each neuron, compute Σ(weight × pre_activation).
3. **Squash**: apply tanh to clamp to [-1, 1].
4. **Compare**: prediction vs. observation → surprise signal → Neurogenesis.
5. **Clean**: remove orphan activations (no synapses → dead).

**HebbianLearningSystem** — STDP weight adaptation. Every tick:

1. **STDP**: Δw = η · pre_activation · post_activation (η = 0.01).
2. **Decay**: w = (w + Δw) × λ (λ = 0.999).
3. **Micro-prune**: if w < 0.001, remove the synapse immediately (no need to wait for the 1 s apoptosis sweep).
4. **Gossip**: every N ticks, serialise accumulated gradients into a FlatBuffer frame and dispatch to up to 3 DHT peers (cluster-biased: same-type nodes first).

### Neurogenesis: Surprise-Driven Neuron Birth

Apoptosis is timer-based (Phase 5, ~1 s); birth must be reactive. Neurogenesis is triggered by **prediction error** (surprise) from the ForwardPassSystem:

```
accumulator[t+1] = (accumulator[t] + error_delta) × decay_rate
```

- Default threshold: 0.2 (20 % accumulated error).
- Default decay: 0.99 per tick (half-life ≈ 69 ticks).
- Transient noise → decays away, no spawn.
- Sustained novelty → accumulator fills → new neuron spawned → accumulator reset.

Each new neuron is assigned a random 256-bit `EntityId` with inbound/outbound synapses initialised to random weights in [-0.1, 0.1].

### Apoptosis: Programmed Neuron Death

Apoptosis runs every ~1 s during Phase 5. It enforces four pruning criteria:

| Target | Criteria | Effect |
|---|---|---|
| DHT routing entry | `fail_count ≥ 3` | Remove from k-bucket |
| DHT routing entry | `latency > 500 ms` | Remove from k-bucket |
| DHT routing entry | `last_seen > 600 s` (10 min) | Remove from k-bucket |
| Pending PING | `age > 10 s` | Remove from pending map |
| Reliable DATA frame | `weight < 0.001` (stale) | Remove from send queue |
| Orphaned frames | Tied to evicted DHT node | Remove from send queue |

**Death spiral guard**: if a single sweep evicts more than a configurable fraction of the routing table, the engine emits a warning. This catches network partitions or mass node failure before the routing table collapses entirely.

---

## Key Numbers

| Metric | Value |
|---|---|
| Rust source files | 17 |
| Total lines of Rust | 7,975 (17 source files) |
| Test functions | 77 |
| Test pass rate | 77 / 77 (100 %) |
| Pre-existing warnings | 0 (zero-warnings policy enforced) |
| External dependencies | 5 (crc32fast, rand, serde, toml, csv) |
| Subsystems | 7 |
| Engine tick rate | ~400 KHz–1 MHz (commodity x86-64) |
| Transport header size | 16 bytes |
| NWP header size | 16 bytes |
| DHT bucket size (K) | 20 |
| DHT buckets | 256 |
| Node ID size | 256 bits |
| DHT bootstrap methods | 4 (cache, DNS, hardcoded, passive) |
| Reliability tiers | 3 (BestEffort, Data, Consensus) |
| ACK bitfield window | 33 packets per ACK |
| Default gradient half-life | 100 ms |
| Hebbian learning rate (η) | 0.01 |
| Weight decay (λ) | 0.999 |
| Activation leak | 0.95 |
| Neurogenesis threshold | 0.2 (surprise accumulator) |
| Apoptosis sweep interval | 1 s (every 1000 ticks) |
| Max failures before eviction | 3 |
| Stale peer timeout | 600 s (10 min) |
| Git commit | `095f119` |
| Build profile (release) | `opt-level = "z"`, LTO, strip |

### Subsystem Test Coverage

| Subsystem | Tests | File(s) |
|---|---|---|
| DHT Routing | 7 | [`dht.rs`](src/dht.rs) |
| UDP Transport | 10 | [`transport.rs`](src/transport.rs), [`udp_transport.rs`](src/udp_transport.rs) |
| ForwardPassSystem | 7 | [`forward_pass.rs`](src/forward_pass.rs) |
| HebbianLearningSystem | 8 | [`hebbian.rs`](src/hebbian.rs) |
| NeurogenesisSystem | 8 | [`neurogenesis.rs`](src/neurogenesis.rs) |
| ApoptosisSystem | 5 | [`apoptosis.rs`](src/apoptosis.rs) |
| EngineLoop | 4 | [`engine_loop.rs`](src/engine_loop.rs) |
| Supporting modules | 11 | (components, flat, zerocopy, simulator, network, etc.) |

---

## Reproducibility

> 🔬 Full guide: [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md)  
> 🚀 One-command reproduction: `bash scripts/repro.sh`  
> 📊 Live dashboard: [`neuron-wire-dashboard.vercel.app`](https://neuron-wire-dashboard.vercel.app)

Every result from this codebase is traceable to a specific execution environment.

### Build

```bash
git clone <repo>
cd neuron-wire
cargo build --release     # opt-level=z, LTO, stripped
```

The release build produces a single statically linked binary. No external runtime dependencies.

### Tests

```bash
cargo test                # 125 tests, 0 failures
```

All tests are deterministic (seeded RNG, no wall-clock-dependent assertions in the unit test suite). Zero warnings enforced across the entire codebase.

### Paper Mode Simulation

```bash
cargo run --example simulate -- --paper-mode --nodes 10 --duration 120 \
    --output-dir results/experiment-001
```

When `--paper-mode` is set:

- **All RNG seeded** from a single fixed seed.
- **All parameters frozen** into `experiment.toml`.
- **Expected outputs verified** against known-good CSVs.
- **Timestamps are relative**, not absolute — no platform dependency.
- **Git commit, rustc version, and platform info** are captured in `metadata.json`.

Output structure:

```
results/experiment-001/
├── experiment.toml       # Frozen parameter config
├── metadata.json         # Git commit, rustc, platform, timestamps
├── convergence.csv       # Per-tick gradient variance across all nodes
├── bandwidth.csv         # Aggregate bytes in/out per second
├── routing.csv           # DHT peer count per node over time
├── apoptosis.csv         # Death counts per sweep
├── summary.csv           # One-row aggregation per trial
└── raw/                  # Per-node event logs
    ├── node_000.jsonl
    ├── node_001.jsonl
    └── ...
```

### Dependency Tree

| Crate | Version | Purpose |
|---|---|---|
| [`crc32fast`](https://crates.io/crates/crc32fast) | 1 | Header integrity checksums (SIMD-accelerated) |
| [`rand`](https://crates.io/crates/rand) | 0.8 | Entropy for node IDs, synapse weights, gossip selection |
| [`serde`](https://crates.io/crates/serde) | 1 | Simulation config and metadata serialisation |
| [`toml`](https://crates.io/crates/toml) | 0.7 | Experiment configuration parsing |
| [`csv`](https://crates.io/crates/csv) | 1 | Time-series data export for analysis |

Total transitive dependency count: **~25 crates** (vs. 50+ for an equivalent Tokio-based runtime).

---

## Limitations

NWP v0.2.0 is a functional research prototype. The following limitations are known and actively being addressed:

1. **No NAT traversal** — all nodes must be directly reachable on a UDP port. STUN/TURN/ICE are not implemented. This limits deployment to VPS or LAN environments.

2. **No encryption** — the wire format has no transport-layer security. All messages are sent in plaintext. A malicious peer can read, inject, or drop frames. DTLS or Noise Protocol integration is planned.

3. **Single-threaded engine** — while adequate for ~1 MHz tick rates on a single core, the engine cannot exploit multi-core systems. A multi-threaded sharded engine is on the roadmap for networks exceeding 10⁴ neurons.

4. **No persistent storage** — the activation map, synapse map, and routing table live entirely in memory. There is no snapshot/restore mechanism. Node restart = total state loss.

5. **DHT-only discovery** — the protocol relies entirely on the Kademlia DHT for peer discovery. There is no mDNS, no LAN broadcast, and no rendezvous server fallback beyond the hardcoded seed list.

6. **Gradient gossip fanout is static** — Hebbian gossip sends to at most 3 peers. In a network of 10³+ nodes, this fanout may be insufficient for convergence. Adaptive fanout based on network size is an open research question.

7. **No Byzantine fault tolerance** — the consensus mechanism is not BFT. A coalition of malicious nodes controlling >⅓ of the routing table can partition the network.

8. **Linux/x86-64 only (tested)** — the UDP transport uses `std::net::UdpSocket` which should be portable, but the project has only been tested on x86-64 Linux. Windows and macOS builds may exhibit different timing characteristics.

9. **Zero-warnings policy enforced** — the codebase compiles with `cargo clippy -D warnings` in CI, ensuring no warnings in any module.

10. **Documentation coverage** — the inline module-level documentation is thorough, but high-level architectural documentation (this README) was written retroactively. Some internal APIs lack doc examples.

---

## Documentation

Beyond this README, the following documents provide deeper coverage of specific aspects:

| Document | What it covers |
|----------|----------------|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Full subsystem design, engine loop phases, data flow, design decisions |
| [`FOUNDATIONAL_QNA.md`](FOUNDATIONAL_QNA.md) | 20 categories of research questions answered from codebase evidence (includes glossary) |
| [`ROADMAP.md`](ROADMAP.md) | 20-direction research plan with effort estimates and priority matrix |
| [`LESSONS_LEARNED.md`](LESSONS_LEARNED.md) | Personal reflections on hypothesis failure, reproducibility, and process |
| [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) | Detailed guide for reproducing every benchmark |
| [`DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md) | Codebase tour, module dependencies, testing patterns, debugging, profiling |
| [`PROTOCOL_SPEC.md`](PROTOCOL_SPEC.md) | Formal wire format, message types, handshake, DHT routing, reliability |
| [`PUBLISHING_CHECKLIST.md`](PUBLISHING_CHECKLIST.md) | Level 1–5 readiness inventory for publication |
| [`PAPER.md`](PAPER.md) | Paper outline and target venues |

For API-level documentation, build with `cargo doc --no-deps --open`.

---

## Roadmap

### v0.3.0 — Hardening & Real-World Networking
- [ ] STUN-based NAT traversal
- [ ] Noise Protocol encryption (wire-level, zero-alloc)
- [ ] Peer cache persistence across restarts (filesystem-backed)
- [ ] Configurable gradient half-life per message type
- [ ] Automated benchmark suite (latency, throughput, convergence)

### v0.4.0 — Scalability
- [ ] Multi-threaded sharded engine (one shard per core, shard-local routing table)
- [ ] Adaptive gossip fanout (proportional to `log₂(network_size)`)
- [ ] Sparse gradient compression (top-k sparsification before gossip)
- [ ] Deterministic snapshot/restore (full state serialisation)

### v0.5.0 — Advanced Neural Computation
- [ ] LSTM-style gated memory cells as first-class neuron types
- [ ] Homeostatic plasticity (adjust firing thresholds to maintain target activity)
- [ ] Structural plasticity (dendritic spine growth: create synapses without spawning a new neuron)
- [ ] Reward-modulated Hebbian learning (3-factor learning rule)

### v0.6.0 — Research Platform
- [ ] Reproducible experiment harness (paper-mode simulation with CSV exports — partially implemented in simulator.rs)
- [ ] Prometheus metrics export (tick rate, peer count, synapse count, gradient throughput)
- [ ] Python bindings (PyO3) for integration with ML research tooling
- [ ] Published paper(s) with accompanying reproduction package

### Future Directions
- **Hierarchical DHT**: multiple routing layers for geographical vs. topical proximity.
- **Gossip-based consensus**: replace the current simple majority vote with a BFT gossip protocol (HoneyBadgerBFT or similar).
- **Differentiable routing**: learn routing weights end-to-end using gradient information.
- **Hardware acceleration**: offload tanh squashing and STDP updates to GPU or SIMD.

---

## License

[MIT](LICENSE-MIT) © 2025–2026 Neuron Wire Contributors

*Built with Rust 2021 edition. Single-threaded, non-blocking, and intentionally free of async runtimes.*
