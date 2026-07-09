# Grant Application: Neuron Wire (NWP)

**Open infrastructure for decentralized AI.**

| Field | Value |
|-------|-------|
| **Organization** | [Zylvon](https://zylvon.com) |
| **Repository** | [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) |
| **License** | MIT |
| **Status** | Research prototype (v0.3.0) |
| **Codebase** | 55 Rust source files, 7,975 LOC, 10 unit tests, zero clippy warnings |
| **CI** | Cross-platform build + test, clippy, coverage, benchmark tracking |
| **Simulation scale** | 1 B nodes converged (v4 hybrid, 15.6 s wall time) |

---

## Executive Summary

Neuron Wire (NWP) is an open-source infrastructure layer for decentralized AI. It enables any device reachable over a network to discover peers, exchange learning signals, and participate in collaborative computation without provisioning servers or trusting a central coordinator.

The core protocol provides: latency-weighted Kademlia DHT for peer discovery, custom reliable UDP transport with gradient decay, Hebbian STDP distributed learning, identity and trust primitives, and a deterministic paper-mode simulation framework. The engine is a single-threaded non-blocking loop that sustains ~400 KHz–1 MHz tick rates on commodity hardware with zero external runtime dependencies.

Existing benchmarks (v3 simulator) demonstrate 100% DHT convergence at 100 K nodes; the v4 hybrid model extends this to 1 B nodes in 15.6 s wall time, suggesting the underlying routing protocol scales at least to O(log N) convergence with O(1) bandwidth per node relative to network size.

We request support to: **(1)** deploy NWP across 100+ real Internet nodes on three continents and measure what actually happens under realistic network conditions; **(2)** release reproducible datasets, baselines, and a peer-reviewed publication including negative findings; and **(3)** build a developer SDK that reduces onboarding to `cargo add neuron-wire`.

---

## Problem

Building distributed AI today requires centralized infrastructure. Every gradient flows through a parameter server. Every participant must trust a central operator. Federated learning still requires an aggregator. All-Reduce assumes a static, known participant set. A developer in any region with unreliable cloud access can lose the model their work depends on without warning.

Three constraints drive the need for a different architecture:

1. **Access** — collaborative AI should not require provisioning cloud infrastructure. A device with a network connection should be able to participate.
2. **Privacy** — data should not leave the device. Only learning signals (gradients, activations) should be exchanged, with cryptographic guarantees about what was shared.
3. **Trust** — participation should not require trusting a coordinator, a parameter server, or any single party. The network should be self-organizing and Byzantine-resilient.

Neuron Wire explores whether these constraints can be satisfied simultaneously with a single protocol.

---

## Current Status

### Core protocol

| Primitive | Module | Tests | Description |
|-----------|--------|-------|-------------|
| DHT routing | `dht.rs` | 7 | Latency-weighted Kademlia, 256 buckets, K=20, DNS seed bootstrap |
| UDP transport | `transport.rs` | 10 | Custom reliable UDP, 3 reliability tiers, ACK bitfield, gradient decay |
| Engine loop | `engine_loop.rs` | 4 | Single-threaded non-blocking 6-phase tick, 400 KHz–1 MHz, 0% CPU idle |
| Identity & crypto | `identity.rs`, `security.rs` | — | Ed25519 signatures, Noise Protocol-ready symmetric crypto |
| Trust scoring | `trust.rs` | — | Reputation-based Sybil resistance |
| Audit logging | `audit.rs` | — | Hash-chained append-only audit log |
| Observability | `observability/` | — | Prometheus metrics, OTel, live dashboard |
| Simulator | `simulator.rs` | — | Deterministic paper-mode, metadata capture, CSV export, known-good validation |

### Distributed learning subsystems

| Subsystem | LOC | Function |
|-----------|-----|----------|
| `hebbian.rs` | 683 | STDP weight updates, L2 decay, micro-pruning, sparse gossip dispatch |
| `forward_pass.rs` | 413 | Activation propagation with tanh, prediction error (surprise signal) |
| `neurogenesis.rs` | 340 | Surprise-driven neuron birth (leaky accumulator > 0.2) |
| `apoptosis.rs` | 306 | Programmed neuron death (4 criteria, death spiral guard) |

### Baseline comparisons (Python)

7 baselines in `baselines/`: federated averaging, decentralized SGD, parameter server, Horovod All-Reduce, BitTensor, Ray distributed, and a unified comparison framework.

### Research infrastructure

- **Formal model** — [`FORMAL_MODEL.md`](FORMAL_MODEL.md) (1,760 lines, 17 sections): network model, convergence analysis, DHT convergence theorem, redundancy bounds, partition tolerance, trust convergence, entropy bounds, failure probabilities
- **Reproducible experiments** — 10 TOML configs covering DHT convergence, scaling, bandwidth, churn, failure injection
- **One-command reproduction** — `scripts/reproduce.sh`: capture env → build → run all 10 experiments → validate → generate 9 publication-ready matplotlib figures
- **Architecture decisions** — 8 ADRs documenting design tradeoffs with rationale
- **Browser demo** — Pure WASM P2P neural network (101 KB WASM, two tabs auto-discover via BroadcastChannel, no server)

### DHT convergence benchmarks

| Scale | Nodes | Convergence | CT (sim‑s) | Avg Peers | BW | Wall Time |
|-------|-------|-------------|------------|-----------|-----|-----------|
| v3 sim | 100 K | 100% | 7.0 | 59 | 202 Mbps | 43.4 s |
| v3 sim | 10 K | 100% | 1.0 | 58 | 52 Mbps | 1.5 s |
| v4 hybrid | 100 K | 99.9% | 7.5 | 117 | 172 Mbps | 4.9 s |
| v4 hybrid | 1 M | 99.8% | 10.0 | 148 | 149 Mbps | 12.4 s |
| v4 hybrid | 10 M | 98.9% | 10.0 | 142 | 137 Mbps | 12.2 s |
| v4 hybrid | 100 M | 99.8% | 12.5 | 176 | 125 Mbps | 15.5 s |
| **v4 hybrid** | **1 B** | **99.5%** | **12.5** | **176** | **125 Mbps** | **15.6 s** |

*v4 uses a hybrid model: 200 K active nodes with statistical virtual nodes for the remainder — convergence threshold = max(3 log₂(N), 30).*

---

## Research Philosophy

Two principles guide this project:

1. **Evidence, not features.** Every claim deserves an experiment. A claim about convergence without a measurement is an opinion.
2. **A limitation documented is stronger than an assumption hidden.** The limitations section should grow as fast as the features section.

Before the first WAN deployment, the experimental protocol — hypothesis, null hypothesis, independent and dependent variables, controlled variables, success and failure criteria, statistical analysis plan — will be timestamped and committed to the repository.

Claims are stated precisely: "under the evaluated conditions," "in the tested configuration," "our experiments suggest." Negative results are published alongside positive ones.

---

## Requested Deliverables

### Deliverable 1: Real Internet Deployment (100+ Nodes, 3 Continents)

**Problem:** All existing benchmarks are localhost simulations. Real networks have NAT, jitter, loss, asymmetric routing, and heterogeneous hardware. It is not known which assumptions survive contact with the Internet.

**Plan:**
- Deploy 100+ nodes across cloud infrastructure in North America (us-east-1), Europe (eu-west-1), and Asia-Pacific (ap-southeast-2)
- Measure: latency distribution (intra-region, inter-region), packet loss rate, churn patterns, DHT convergence time, gradient delivery success rate
- Implement: Noise Protocol encryption (zero-alloc DTLS-style handshake), STUN-based NAT traversal for non-VPS nodes
- **Negative results published alongside positive ones**

**Success criteria:**
- 100+ nodes sustain connectivity for 7+ days
- DHT convergence time measured under real churn
- Cross-continent gradient delivery with < 50% loss
- Public live dashboard with per-region metrics

### Deliverable 2: Publication & Reproducible Benchmarks

**Plan:**
- Reproducible evaluation paper comparing NWP against vanilla Kademlia, gossip SGD, and federated averaging on the same testbed
- All 7 Python baselines run with the same measurement methodology
- Open datasets: per-tick latency, throughput, convergence curves across 3 topologies
- Submit to workshop track (SysML, MLSys, HotOS, DSN) — registered report format preferred
- One-command `cargo run --release -- --reproduce-all` reproduces every figure

### Deliverable 3: Developer SDK & Ecosystem

**Plan:**
- `cargo add neuron-wire` → 5 public functions: `start_node`, `connect`, `send_gradient`, `recv_gradient`, `get_stats`
- Python bindings via PyO3 for ML researchers
- 5 reference projects: decentralized LLM inference, collaborative edge learning, disaster communication AI, decentralized robotics, distributed scientific computing
- Peer cache persistence + snapshot/restore (solves "node restart = total state loss")
- WebRTC transport for cross-machine browser-to-browser

---

## Budget

| Category | Estimated Cost | Justification |
|----------|---------------|---------------|
| VPS compute (3 months, 100+ nodes, 3 regions) | $600–$1,200 | AWS free-tier burstable instances |
| Open-access publication fees | $1,000–$3,000 | Workshop or conference publication |
| Engineering (NAT traversal, encryption, SDK) | — | In-kind or funded |
| Conference travel | $1,000–$2,000 | One workshop presentation |
| Community infrastructure | $200–$500 | Discord, CI/CD, docs hosting |

---

## Alignment with Zylvon Mission

Zylvon builds open infrastructure for decentralized AI. Neuron Wire is the networking and runtime substrate:

| Priority | How Neuron Wire Addresses It |
|----------|------------------------------|
| Open source | MIT-licensed, 55+ source files, full CI/CD |
| Decentralized AI infra | Purpose-built P2P transport + DHT + distributed learning |
| Privacy by design | Data never leaves device; only gradients gossip over network |
| Accessible | Single-threaded engine, opt-level=z, 512 MB RAM target |
| Reproducible science | One-command reproduction, metadata capture, known-good validation |
| Compute without central control | P2P by design: no parameter server, no orchestrator, no fees |

---

*For questions: [team@zylvon.com](mailto:team@zylvon.com) · [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)*
