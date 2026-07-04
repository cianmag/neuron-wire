# Grant Application: Neuron Wire (NWP)

**Zero-Infrastructure AI** — An open-source infrastructure layer for decentralized AI systems.

> **Target:** Sentient Foundation — Open Source AGI Grant Programme  
> **Track:** Grants (no strings — public goods builder)  
> **Amount requested:** $25,000 (Grant track)  
> **Status:** Draft — ready for submission review

---

## One-Sentence Pitch

**Neuron Wire is an open-source infrastructure layer for decentralized AI that enables any device to discover peers, exchange learning signals, and participate in collaborative AI without provisioning servers or trusting a central coordinator.**

---

## Executive Summary

Today, building distributed AI requires cloud infrastructure, orchestration, and centralized coordination. Every gradient flows through a parameter server. Every participant must trust a central operator. A developer in Lagos, Bangalore, or São Paulo can lose the model their work depends on without warning.

Neuron Wire (NWP) explores a different model: **any device that can reach another device should be able to join a collaborative AI network without provisioning servers or trusting a central coordinator.** We have built the networking and runtime substrate required to test whether this vision is practical — 42 source modules, 19,220 lines of Rust, 256 tests, 8 architecture decision records, 6 tutorials, 7 baseline comparisons, 10 reproducible experiment configurations, and a working browser-based P2P demo.

We are requesting support to:

1. **Deploy neuron-wire across 100+ real Internet nodes on multiple continents** — measure latency, churn, and convergence under realistic network conditions for the first time.
2. **Release reproducible datasets, benchmarks, and a peer-reviewed publication** — including negative findings.
3. **Build a developer SDK that makes neuron-wire installable in five minutes** — `cargo add neuron-wire`, Python bindings, and documented integration patterns for five potential applications.

---

## Application History & Status

| Aspect | Status |
|--------|--------|
| **Repository** | [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) |
| **Lines of Rust** | 19,220 across 55 source files |
| **Tests** | 256 `#[test]` annotations across 35/42 modules + integration + property + security + stress |
| **Build** | `cargo check -D warnings` clean, `cargo clippy` 0 errors |
| **Documentation** | mdBook, 6 tutorials, 8 ADRs, formal mathematical model (1,760 lines) |
| **CI** | 4 workflows (build+test, docs, release, reproduction validation) |
| **License** | MIT |
| **Published crates** | Not yet — pending SDK stabilization |
| **WASM demo** | Working P2P browser demo (BroadcastChannel, no server) |
| **Years building** | 11 days of active development, 60 commits, single contributor |

---

## What We've Built

Neuron Wire is an infrastructure layer — the operating system for decentralized AI. It provides the primitives that every decentralized AI application needs and that no existing library provides together in a single, auditable runtime:

### Core Infrastructure Primitives

| Primitive | Module | What it does | Status |
|-----------|--------|-------------|--------|
| **Peer discovery** | `src/dht.rs` (1,209 lines) | Latency-weighted Kademlia DHT — 256 buckets, K=20, DNS seed bootstrap | ✅ Tested 7 unit tests |
| **Secure transport** | `src/transport.rs` (642 lines) | Custom UDP protocol with 3 reliability tiers, ACK bitfield, gradient decay | ✅ Tested 10 unit tests |
| **Engine loop** | `src/engine_loop.rs` (1,045 lines) | Single-threaded non-blocking 6-phase tick, ~400 KHz–1 MHz, 0% CPU when idle | ✅ Tested 4 unit tests |
| **Identity & crypto** | `src/identity.rs`, `src/secure_channel.rs`, `src/security.rs` | Ed25519 signatures, Noise Protocol-ready symmetric crypto, zeroize | ✅ |
| **Trust scoring** | `src/trust.rs` (504 lines) | Reputation-based Sybil resistance | ✅ Tested |
| **Audit logging** | `src/audit.rs` (534 lines) | Hash-chained append-only audit log | ✅ Tested |
| **Observability** | `src/observability/` (4 files) | Prometheus metrics, OpenTelemetry, live dashboard | ✅ |
| **Simulator** | `src/simulator.rs` (1,515 lines) | Deterministic paper-mode with metadata capture, CSV export, known-good validation | ✅ 10 experiment configs |

### Distributed Learning Subsystems

| Subsystem | Lines | Description |
|-----------|-------|-------------|
| `src/hebbian.rs` | 683 | STDP weight updates, L2 decay, micro-pruning, sparse gossip dispatch |
| `src/forward_pass.rs` | 413 | Activation propagation with tanh squashing, prediction error (surprise) |
| `src/neurogenesis.rs` | 340 | Surprise-driven neuron birth (leaky accumulator > 0.2) |
| `src/apoptosis.rs` | 306 | Programmed neuron death (4 pruning criteria, death spiral guard) |
| `src/attention.rs` | 466 | Attention mechanism for weighted gradient exchange |
| `src/ml.rs` | 389 | ML module integration layer |
| `src/meta_learning.rs` | 337 | Meta-learning parameter adaptation |
| `src/curiosity.rs` | 265 | Intrinsic motivation / exploration drive |
| `src/dynamic_activation.rs` | 380 | Dynamic activation function selection |

### Baseline Comparisons (Python)

| Baseline | File | Purpose |
|----------|------|---------|
| Federated averaging | `baselines/federated.py` | Centralized coordinator baseline |
| Decentralized SGD | `baselines/decentralized_sgd.py` | Gossip-based parameter averaging |
| Parameter server | `baselines/parameter_server.py` | Traditional PS architecture |
| All-Reduce (Horovod) | `baselines/horovod_baseline.py` | Ring-AllReduce baseline |
| BitTensor | `baselines/bittensor_baseline.py` | Incentivized subnet comparison |
| Ray distributed | `baselines/ray_baseline.py` | Ray remote function baseline |
| Comparison framework | `baselines/comparison_framework.py` | Unified evaluation harness across all baselines |

### Research Infrastructure

- **Formal mathematical model** — `FORMAL_MODEL.md` (1,760 lines, 17 sections): network model, convergence analysis, DHT convergence theorem, redundancy bounds, partition tolerance analysis, trust convergence analysis, entropy bounds, failure probabilities
- **Reproducible experiments** — 10 TOML configs covering DHT convergence, scaling, bandwidth, churn, failure injection, multi-trial stats, SGA comparison
- **One-command reproduction** — `scripts/reproduce.sh` captures env → builds → runs all 10 experiments → validates against known-good → generates 9 publication-ready matplotlib figures
- **Tutorial series** — 6 tutorials covering everything from first node to deep observability (2,010 lines)
- **Architecture decisions** — 8 ADRs documenting every design tradeoff with rationale

### Browser Demo (Tier 10)

A pure-WASM P2P neural network that runs by opening a URL. Two tabs auto-discover each other via BroadcastChannel, exchange activations, and learn together. No server. No install.

- **563-line WASM engine** — 6 neurons, Hebbian STDP, heartbeat discovery, gradient exchange
- **456-line JS renderer** — Canvas 2D with glow effects, particle bursts, packet animations
- **101KB WASM binary** — compiled with wasm-pack, serves from any static host

---

## Why Neuron Wire Aligns with Sentient's Mission

Sentient Foundation funds projects that are open, accessible, private, empowering, and good for humanity. Neuron Wire was not built for this grant — but it aligns with every stated priority:

| Sentient Priority | How We Meet It |
|------------------|----------------|
| **Open source** | MIT-licensed. 55 Rust source files, full CI/CD with 4 workflows, issue templates, contributing guide. Anyone can inspect, modify, and redistribute every line. |
| **Yours to keep** | Open weights, once released, can never be taken back. Neuron Wire makes it possible to run collaborative AI without depending on any API key, any cloud provider, or any single organization. |
| **Accessible** | Runs on hardware people actually own — single-threaded engine, `opt-level="z"`, 512 MB RAM target. The WASM demo runs in any browser tab on any device. |
| **Private by default** | Data never leaves the device. Only activation gradients are exchanged over the network, decayed exponentially over 100 ms half-life. The hash-chained audit log provides verifiable proof of what was shared. |
| **Empowering, not extractive** | Any device with a UDP port can join. No coordinator, no fees, no central authority can revoke access. The network belongs to its participants. |
| **Good for humanity** | Potential applications: decentralized LLM inference for underserved languages, collaborative edge learning for medical imaging in low-infrastructure regions, disaster communication AI when infrastructure is destroyed. These are directions the infrastructure enables, not features we have demonstrated. |
| **Decentralized AI infrastructure** | This is the explicit design goal. Not an ML framework retrofitted for distribution, but a distributed systems protocol built from the ground up with AI workloads as its first-class concern. |
| **Identity & verification** | Ed25519 signatures on every message, trust scoring for Sybil resistance, hash-chained audit log for verifiable provenance. |
| **Compute without central control** | P2P by design — no parameter server, no orchestrator, no single point of failure or control. |

---

## Research Philosophy

Two sentences guide every decision in this project:

> **Evidence, not features.** Every claim deserves an experiment.

When planning experiments, we ask: *Can I measure it? Can someone else reproduce it? Would a skeptical reviewer believe it? Does it answer our research question?* If the answer is "no," it doesn't belong in v1.0.

Success will not be measured by GitHub stars or downloads. Success means producing reproducible evidence about decentralized collaborative learning that other researchers can independently validate, reproduce, and extend.

Before deploying a single node across continents, the experimental protocol — hypothesis, null hypothesis, independent and dependent variables, controlled variables, success and failure criteria, statistical analysis plan — will be written, timestamped, and committed to the repository. This prevents rewriting hypotheses to fit results after seeing them.

When reporting findings, the language is precise: "our experiments suggest," "under the evaluated conditions," and "in the tested configuration." Cautious claims are believed more readily than sweeping ones, and the project is strongest when it is understated.

## Requested Deliverables

### Deliverable 1: Real Internet Deployment (100+ Nodes, 3 Continents)

**Problem:** Every benchmark so far is localhost UDP. Real networks have NAT, jitter, loss, asymmetric routing, and heterogeneous hardware. We don't know if any of our assumptions survive contact with the real Internet.

**Plan:**
- Deploy 100+ neuron-wire nodes across cloud infrastructure in North America (us-east-1), Europe (eu-west-1), and Asia-Pacific (ap-southeast-2)
- Each node runs the standard engine loop with DHT routing, gradient exchange, and observability pipeline
- Collect: latency distribution (intra-region, inter-region), packet loss rate, churn patterns, DHT convergence time, gradient delivery success rate
- Implement: Noise Protocol encryption (DTLS-style, zero-alloc), STUN-based NAT traversal for non-VPS participation
- **Negative results published alongside positive ones** — this is the first real-world test of the architecture

**Success criteria:**
- 100+ nodes sustain connectivity for 7+ days
- DHT convergence time measured under real churn (NAT rebind, VM restart, packet loss)
- Cross-continent gradient delivery with < 50% loss
- Public live dashboard showing per-region metrics

**Budget use:** VPS compute ($200-400/month × 3 months), engineering time for NAT traversal + wire encryption

### Deliverable 2: Publication & Reproducible Benchmarks

**Problem:** No decentralized AI infrastructure project has published a reproducible evaluation with open datasets. This is why the field is full of claims and empty of evidence.

**Plan:**
- Write a reproducible evaluation paper comparing neuron-wire against: vanilla Kademlia (no latency weighting), gossip SGD (random peer selection), federated averaging (central coordinator)
- All 7 Python baselines already exist in `baselines/` — run them on the same testbed with the same measurement methodology
- Generate open datasets: per-tick latency, throughput, convergence curves for all 4 systems across 3 deployment topologies (LAN, WAN, mixed)
- Submit to workshop track (SysML, MLSys, HotOS, DSN) — registered report format preferred (results-blind review)
- One-command `cargo run --release -- --reproduce-all` reproduces every figure in the paper

**Success criteria:**
- Paper accepted at workshop or conference
- Datasets published under open license (CC-BY)
- All figures one-command reproducible from a fresh clone

**Budget use:** Open-access publication fees, compute for baseline experiments, potential conference travel

### Deliverable 3: Developer SDK & Ecosystem

**Problem:** Neuron Wire is 19,000 lines of Rust with no onboarding path. The fastest way to use it is to clone the repo and read the examples. This limits adoption to other Rust systems programmers.

**Plan:**
- `cargo add neuron-wire` — publish to crates.io with minimal API surface (5 public functions: `start_node`, `connect`, `send_gradient`, `recv_gradient`, `get_stats`)
- Python bindings via PyO3 — ML researchers can import neuron-wire from Python and use it with numpy arrays
- Documentation: "Five projects built on neuron-wire" — decentralized LLM inference, collaborative edge learning, disaster communication AI, decentralized robotics, distributed scientific computing
- Peer cache persistence (filesystem-backed) and snapshot/restore (full state serialization) — solve the "node restart = total state loss" limitation
- WebRTC transport for browser-to-browser across machines (extends the WASM demo beyond BroadcastChannel)

**Success criteria:**
- `cargo add neuron-wire` → five minutes to a running node
- Python wheel published on PyPI
- At least 2 external contributors from other research groups

**Budget use:** Engineering time for PyO3 bindings, SDK polish, documentation, community building

---

## Why Us

**We have built,** in 11 days of active development, an infrastructure project that normally takes teams months:

- 42 source modules • 19,220 lines of Rust • 256 tests • 0 clippy errors
- Formal mathematical model (1,760 lines): convergence proofs, complexity bounds, failure probabilities
- 7 baseline comparisons in Python against established distributed ML architectures
- 10 reproducible experiment configs with one-command reproduction
- 6 tutorials, 8 architecture decision records, 4 CI workflows
- A working browser-based P2P demo (100KB WASM, two tabs, no server)

**But more importantly:** we are treating this as research, not marketing. The repository documents negative results (LESSONS_LEARNED.md), limitations (README.md §Limitations), and open research questions (FOUNDATIONAL_QNA.md). The formal model includes proven bounds on convergence, redundancy, and partition tolerance — not hand-wavy claims.

**Our strongest advantage** is not that we built a technically ambitious open-source systems project. It's that we're willing to publish findings — including negative ones — and let the experiments reveal what kinds of decentralized AI are practical, rather than promising what we can't deliver.

---

## Budget

> *To be completed based on Sentient Foundation guidance. Indicative ranges below.*

| Category | Estimated Cost | Justification |
|----------|---------------|---------------|
| VPS compute (3 months, 100+ nodes) | $600–$1,200 | AWS free-tier burstable instances across 3 regions |
| Open-access publication fees | $1,000–$3,000 | Workshop/conference publication |
| Engineering (NAT traversal, encryption, SDK) | — | In-kind (primary contributor) or funded |
| Conference travel (if applicable) | $1,000–$2,000 | One workshop or conference presentation |
| Community infrastructure | $200–$500 | Discord, CI/CD, documentation hosting |

---

## Application Checklist

Before submitting, verify:

- [ ] Repository is public ✅ ([github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire))
- [ ] License is open source ✅ (MIT)
- [ ] Code is buildable and testable from a clean clone ✅
- [ ] CI/CD infrastructure is functional ✅ (4 workflows)
- [ ] Documentation covers architecture, protocol, and developer onboarding ✅
- [ ] Formal mathematical analysis exists ✅ (1,760-line FORMAL_MODEL.md)
- [ ] Baseline comparisons are implemented ✅ (7 Python frameworks)
- [ ] Reproducibility infrastructure is in place ✅ (one-command reproduce)
- [ ] This GRANT.md is complete and ready for submission
