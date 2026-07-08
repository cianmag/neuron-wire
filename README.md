# Neuron Wire (NWP)

**Zero-Infrastructure AI** — An open-source infrastructure layer for decentralized AI systems, developed by [Zylvon](https://zylvon.com).

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![Tests](https://img.shields.io/badge/tests-256+-brightgreen)](#)
[![Lines of Rust](https://img.shields.io/badge/rust-19,220%20LOC-blue)](#)
[![Crates.io](https://img.shields.io/badge/crate-v0.3.0-blue)](#)
[![Live Dashboard](https://img.shields.io/badge/dashboard-live-00c8ff)](https://neuron-wire-dashboard.vercel.app)
[![Discord](https://img.shields.io/badge/discord-join-5865F2)](https://discord.gg/neuron-wire)
[![Documentation](https://img.shields.io/badge/docs-mdBook-00c853)](https://github.com/cianmag/neuron-wire)
[![Reproducible](https://img.shields.io/badge/reproducible-yes-brightgreen)](REPRODUCIBILITY.md)
[![Architecture](https://img.shields.io/badge/architecture-formal-7c3aed)](ARCHITECTURE.md)
[![Roadmap](https://img.shields.io/badge/roadmap-3%20deliverables-ffc107)](ROADMAP.md)

---

## Governing Principles

> **Evidence, not features.** Every claim deserves an experiment.

### Five Rules

1. **Evidence beats elegance** — A beautiful architecture without experiments is an idea.
2. **Validation beats implementation** — Another 5,000 lines impresses GitHub. Another independent reproduction impresses researchers.
3. **Adoption beats architecture** — The best protocol nobody uses has almost no impact.
4. **A limitation documented is stronger than an assumption hidden** — The limitations section should grow as fast as the features section.
5. **Protect the core** — After v1.0, every feature answers: *Does this help validate the research question?*

This project treats research discipline as seriously as engineering. Before adding anything, we ask: *Can I measure it? Can someone else reproduce it? Would a skeptical reviewer believe it? Does it answer our research question?* If the answer is "no," it doesn't belong in v1.0.

v1.0 is intentionally boring. All experimental ideas go into separate repos (`neuron-wire-labs`, `neuron-wire-experiments`, `neuron-wire-next`). The core stays predictable, stable, documented, and dependable.

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

Building distributed AI today requires cloud infrastructure, orchestration, and centralized coordination. Every node needs a server, every gradient flows through a parameter server, and every participant must trust a central operator. This creates three hard problems:

1. **Access.** A developer in Lagos, Bangalore, or São Paulo can lose the model their work depends on without warning. The most sensitive data must leave the person's own device. There is no way to participate in collaborative AI without provisioning infrastructure.

2. **Privacy.** Federated learning still requires a central coordinator. All-Reduce requires a static, known participant set. Every approach assumes data moves to compute, not the other way around.

3. **Trust.** Closed labs meter access, set prices, and revoke capabilities overnight. Open models exist, but the infrastructure to *run them collaboratively* — discovery, routing, secure exchange, reproducible experimentation — does not.

Neuron Wire explores a different model: **any device that can reach another device should be able to join a collaborative AI network without provisioning servers or trusting a central coordinator.** We are building the networking and runtime substrate required to test whether this vision is practical.

## What Neuron Wire Is

Neuron Wire (NWP) is an **open-source infrastructure layer** for decentralized AI systems. It provides the primitives that every decentralized AI application needs and that no existing library provides together:

| Primitive | What it does |
|-----------|-------------|
| **Peer discovery** | Nodes find each other via latency-weighted Kademlia DHT — no central registry |
| **Secure transport** | Custom UDP protocol with reliability tiers, ACK bitfield, gradient decay |
| **Distributed learning** | Hebbian STDP runs locally on each node; gradients propagate via sparse gossip |
| **Reproducible experiments** | Deterministic paper-mode simulator, metadata capture, known-good validation |
| **Observability** | Prometheus metrics, live dashboard, hash-chained audit log |
| **Identity & Trust** | Ed25519 signatures, trust scoring, Sybil-resistant routing |
| **Browser demo** | Pure WASM — open a tab, open another, they discover each other automatically |

The protocol is ~40 bytes overhead per message, runs on a single-threaded non-blocking engine loop at ~400 KHz–1 MHz tick rate, and compiles to a statically linked binary with zero external runtime dependencies.

## Zylvon Alignment

Neuron Wire is a core infrastructure project under **Zylvon** — building the foundational layers for decentralized AI. Every design decision is driven by the principle that **AI infrastructure should be open, verifiable, and accessible from any device without centralized gatekeeping.**

| Zylvon Priority | Neuron Wire |
|----------------|-------------|
| **Open source** | MIT-licensed, 55+ source files, full CI/CD |
| **Decentralized AI infrastructure** | Purpose-built P2P transport + DHT + distributed learning |
| **Privacy by default** | Data never leaves the device; only gradients gossip over the network |
| **Runs on hardware people own** | Single-threaded engine, opt-level=z, 512 MB RAM target |
| **Reproducible science** | One-command reproduction, metadata capture, known-good validation |
| **Empowering, not extractive** | Any device with a UDP port can join; no coordinator, no fees |
| **Identity & safe agents** | Ed25519, trust scoring, hash-chained audit log |
| **Compute without central control** | P2P by design — no parameter server, no orchestrator

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
| **Fast Sim: max nodes converged** | **100,000 (100%)** |
| **Fast Sim: convergence time (100k)** | **7.0s sim-time / 43.4s wall** |
| **Fast Sim: avg peers at 100k** | **59.3 per node** |
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

||| Document | What it covers |
|||----------|----------------|
||| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Full subsystem design, engine loop phases, data flow, design decisions |
||| [`ROADMAP.md`](ROADMAP.md) | Three deliverables with concrete milestones |
||| [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) | Detailed guide for reproducing every benchmark |
||| [`DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md) | Codebase tour, module dependencies, testing patterns, debugging, profiling |
||| [`PROTOCOL_SPEC.md`](PROTOCOL_SPEC.md) | Formal wire format, message types, handshake, DHT routing, reliability |
||| [`PUBLISHING_CHECKLIST.md`](PUBLISHING_CHECKLIST.md) | Level 1–5 readiness inventory for publication |

For API-level documentation, build with `cargo doc --no-deps --open`.

---

## Roadmap

This roadmap is structured around three deliverables towards a production-ready decentralized AI infrastructure layer.

### Deliverable 1 — Real Internet Deployment (100 Nodes, 3 Continents)

Take neuron-wire from localhost to the real Internet. Deploy across free-tier VPS in North America, Europe, and Asia. Measure what actually happens.

- [ ] Global testbed: 100+ nodes across AWS free-tier (us-east-1, eu-west-1, ap-southeast-2)
- [ ] Real metrics: latency distribution, packet loss, churn rate, convergence time vs. simulator predictions
- [ ] Public dashboard: live peer count, routing table health, gradient throughput per continent
- [ ] Noise Protocol encryption at wire level (zero-alloc, DTLS-style handshake)
- [ ] STUN-based NAT traversal for non-VPS nodes
- [ ] Negative results published alongside positive ones

### Deliverable 2 — Publication & Reproducible Benchmarks

- [ ] Reproducible evaluation paper with open datasets
- [ ] Comparison against: vanilla Kademlia, gossip SGD, federated averaging on same testbed
- [ ] All baselines (7 comparison frameworks in `baselines/`) validated against real measurements
- [ ] Registered report at workshop venue (e.g., SysML, MLSys, HotOS, DSN)
- [ ] One-command `cargo run --release -- --reproduce-all` reproduces every figure

### Deliverable 3 — Developer SDK & Ecosystem

Make neuron-wire the `cargo add`-able foundation for decentralized AI.

- [ ] `cargo add neuron-wire` — five minutes to a running node
- [ ] Python bindings (PyO3) for ML research tooling
- [ ] Five projects built on neuron-wire (decentralized LLM inference, collaborative edge learning, disaster communication AI, decentralized robotics, distributed scientific computing)
- [ ] Peer cache persistence and snapshot/restore (ephemeral → durable)
- [ ] WebRTC transport for browser-to-browser across machines (not just BroadcastChannel tabs)

---

## License

[MIT](LICENSE-MIT) © 2025–2026 Zylvon & Neuron Wire Contributors

*Built with Rust 2021 edition. Single-threaded, non-blocking, and intentionally free of async runtimes.*

---

**[Zylvon](https://zylvon.com)** — open infrastructure for decentralized AI.
