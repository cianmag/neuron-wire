# Research Roadmap: From Simulator to Research Artifact

> **Date:** 2026-06-26  
> **Repository:** [neuron-wire](https://github.com/cianmag/neuron-wire)  
> **Status:** L1 ~65% complete — see `PUBLISHING_CHECKLIST.md`

This document maps the 20 directions that would transform neuron-wire from an impressive engineering project into research other people build on.

---

## Contents

- [Publishing Gap (L1 · 1 week)](#1-build-a-real-distributed-testbed)
- [Reinforcement (L2–L3 · 2–4 weeks)](#2-create-a-reproducibility-package)
- [Differentiation (L4 · 1–3 months)](#3-formal-protocol-specification)
- [Breakthrough (L5 · 3–12 months)](#4-develop-a-mathematical-model)

Each item is marked with its primary Level (L1–L5) and estimated effort.

---

## 1. Build a Real Distributed Testbed

**Level:** L1 (Must Have) · **Effort:** 2–3 weeks  
**Why:** Every benchmark so far is localhost UDP. Real networks have NAT, jitter, loss, asymmetric routing, and heterogeneous hardware.

**What it unlocks:**
- Measure real Internet latency, jitter, packet loss, churn
- Compare simulated vs. real-world behavior — the first publishable finding
- Validates (or invalidates) every simulator assumption

**Minimum viable:**
- 3 cheap VPS instances (e.g. $5/mo each) in different regions on different providers
- `neuron-wire-node` binary that joins a seed node on boot
- Log RTT, packet loss, convergence time to CSV
- Compare against simulator at same node count + duration

**Next step:** NAT traversal (QUIC transport hole-punch or relay).

---

## 2. Create a Reproducibility Package

**Level:** L1 (Must Have) · **Effort:** 1 week  
**Why:** A reviewer should be able to reproduce every graph with one command.

**What's needed:**
- `experiments/` directory with `.toml` config files named by figure (`figure4.toml`, `figure7.toml`)
- `reproduce.sh` that runs each experiment and generates plots
- Every commit hash, random seed, compiler version, and system metadata captured
- Generated `.png`/`.svg` output alongside raw CSV

**Already present:** `REPRODUCIBILITY.md`, `scripts/repro.sh`, `scripts/validate_repro.py`, known-good CSVs.  
**Gap:** No `experiments/` directory, no plot generation, one-command reproducibility not tested by a stranger.

---

## 3. Formal Protocol Specification

**Level:** L2 (Research Quality) · **Effort:** 1–2 weeks  
**Why:** RFC-style specs are how protocols get adopted. They let others implement without reading your code.

**What it includes:**
- Packet format byte layouts (can extract from `header.rs`, `flat.rs`, `types.rs`)
- Binary encoding rules for every message type
- Complete state machine diagrams for DHT, retransmission, apoptosis
- Message flow diagrams for bootstrap, gossip, gradient exchange
- Timing diagrams for retransmission, backoff, stale detection
- Error handling for every failure mode
- Version negotiation (MAGIC + VERSION in `lib.rs`)

**Already present:** Mermaid diagrams in `ARCHITECTURE.md` for retransmission and DHT message handling.  
**Gap:** Not extracted into a standalone spec, no binary encoding reference, no error table.

---

## 4. Develop a Mathematical Model

**Level:** L2 (Research Quality) · **Effort:** 2–3 weeks  
**Why:** Big-O complexity is table stakes for systems research. Without it, claims about "scalability" are vibes.

**What to analyze:**
- **Routing complexity:** O(log N) hops in Kademlia — but with K=8 buckets and α=3 parallelism, what's actual distribution?
- **Bandwidth complexity:** Per-node bandwidth as function of N, gossip interval, packet size. Model predicted vs measured.
- **Memory usage:** Per-peer routing entry size, k-bucket storage, gradient buffer. Bounded by K × k-bucket count.
- **Communication cost:** Messages per topology change, per gossip cycle, per bootstrap. Upper bound derivation.
- **Scalability limits:** At what N does bandwidth exceed available capacity? Where does the DHT become the bottleneck?
- **Convergence behavior:** Even an empirical model ("converges in Θ(log N) seconds on LAN") is publishable.

**Already present:** None — no Θ/Ω anywhere in the docs or code.

---

## 5. Perform Large Benchmark Studies

**Level:** L2 (Research Quality) · **Effort:** 3–4 weeks  
**Why:** A few graphs can be cherry-picked. Hundreds of experiments with parameter sweeps cannot.

**Sweep parameters:**
- Node count: 3, 5, 10, 25, 50, 100, 200
- RTT: 0ms (localhost), 10ms, 50ms, 100ms, 200ms
- Packet loss: 0%, 1%, 5%, 10%, 25%
- Churn rate: 0%, 1%/s, 5%/s, 10%/s
- Gossip interval: 100, 500, 1000, 5000 ticks
- Learning rate: 0.001, 0.01, 0.1, 0.5
- Bandwidth limit: unlimited, 1Mbps, 100Kbps per node

**Output:** Heatmaps showing bandwidth, convergence time, peer count as functions of these parameters.

**Already present:** SGA benchmark at 10/25/50 with 2 modes.  
**Gap:** No parameter sweeps, no heatmap generation, no automation.

---

## 6. Build an Interactive Visualizer

**Level:** L2 (Research Quality) · **Effort:** 2–3 weeks  
**Why:** Great visualization makes complex systems understandable. Reviewers scan images before reading text.

**What exists:** `tools/visualizer/index.html` — static p5.js visualizer with 5 demo scenarios.  
**What's missing:**
- Live connection to a running simulation (WebSocket or file watch)
- Real-time edge weight display
- Prediction error timeline
- Highlight routing paths on hover
- Node failure + recovery animation
- Time scrubber to replay experiment

**Minimum viable upgrade:** Make the visualizer read NDJSON from `--trace` output in real time.

---

## 7. Implement Failure Injection

**Level:** L2 (Research Quality) · **Effort:** 1–2 weeks  
**Why:** Systems are defined by how they fail. If you only test happy path, you haven't tested.

**Already present:** `adversary.rs` — corruption, replay, garbage payload, bad CRC, node death, partition, malicious node. 9 tests.  
**Still missing:**
- High latency injection (delay packets before sending)
- Packet duplication
- Clock drift (advance/skew system time for a node)
- Byzantine behavior (lie in routing responses)
- Automatic recovery measurement (time-to-converge after each injection)

---

## 8. Add Security

**Level:** L3 (Engineering Excellence) · **Effort:** 2–4 weeks  
**Why:** If the goal is a real P2P network, it will be attacked. Design for hostile networks from day one.

**What's needed:**
- Node authentication via ed25519 keypair (also needed for persistent identity)
- Signed packets (every message includes a signature)
- Replay protection (nonces or timestamps in every packet)
- Rate limiting (per-peer packet budget)
- Sybil resistance (proof-of-work or stake for NodeId)
- Secure bootstrap (pinned seed node keys)
- Encryption (optional layer — wire encryption via Noise or similar)

**Already present:** None. The adversary module tests *against* attacks but doesn't defend against them.

---

## 9. Build a Public Benchmark Dashboard

**Level:** L3 (Engineering Excellence) · **Effort:** 1–2 weeks  
**Why:** Live metrics prove the system is real. A static README can be faked; a live dashboard cannot.

**What's needed:**
- Prometheus metrics endpoint exporting: active nodes, peers per node, packets/sec, bandwidth, RTT
- Simple dashboard (Grafana or static HTML) displaying live data
- Deploy alongside the distributed testbed

---

## 10. Create an Open Protocol

**Level:** L3 (Engineering Excellence) · **Effort:** 1–2 weeks  
**Why:** Success isn't just "people use my code." It's "people implement my protocol."

**What's needed:**
- Standalone protocol spec document (see #3)
- Wire format as a FlatBuffer schema file (`.fbs`) that generates bindings for any language
- Test vectors: hex-encoded packets with expected parse results
- Interoperability test suite

**Already present:** FlatBuffer-based zero-copy wire format in `flat.rs`. NWP MAGIC/VERSION constants.  
**Gap:** No `.fbs` schema, no test vectors, no standalone spec.

---

## 11. Support Multiple Implementations

**Level:** L3 (Engineering Excellence) · **Effort:** 2–4 weeks  
**Why:** Interoperability demonstrates the protocol, not just one codebase, is sound.

**Ideal targets:**
- **Python simulator** (for education / rapid prototyping) — minimal NWP message parsing + DHT
- **JavaScript visualizer** (already exists as static tool — add real protocol parsing)
- **Go relay node** (lightweight seed node that forwards but doesn't compute)

---

## 12. Publish Datasets

**Level:** L3 (Engineering Excellence) · **Effort:** 1 week  
**Why:** Open datasets encourage replication. They let others test their hypotheses against your data.

**What to publish:**
- All benchmark logs from large sweeps (CSV/JSON)
- Network traces (PCAP or NDJSON of packet-level events)
- Experiment output directories
- Configuration files for every run

**Already present:** Benchmark results checked into `results/`. CI uploads artifacts.  
**Gap:** No DOI or permanent archive, no README for the dataset itself.

---

## 13. Build a Research Website

**Level:** L4 (Research Differentiator) · **Effort:** 2–3 weeks  
**Why:** Makes the project feel like a real research project, not a GitHub repo. First thing reviewers and admissions officers see.

**Domain:** `neuron-wire-dashboard.vercel.app` (already registered in Cargo.toml homepage)  
**What to include:**
- Architecture overview with interactive diagram
- Links to papers (`PAPER.md`, future publications)
- Documentation (rendered from `ARCHITECTURE.md` + `cargo doc`)
- Live benchmarks (embedded from dashboard)
- Interactive demo (embedded visualizer)
- Blog (links to blog series)
- Download / install guide

---

## 14. Write Multiple Papers

**Level:** L4 (Research Differentiator) · **Effort:** Ongoing  
**Why:** Focused papers are stronger than one paper that tries to cover everything.

**Candidate paper topics:**
1. **Runtime architecture** — NWP protocol design, zero-copy FlatBuffer transport, modular subsystems. (Exists as `PAPER.md` sketch.)
2. **Sparse gossip protocol** — mutation-based gradient selection, theoretical bandwidth bound, convergence proof.
3. **DHT routing with SGA** — the surprising finding that maintenance pings contribute zero to routing quality, and what that means for P2P maintenance design.
4. **Failure recovery in adaptive runtimes** — apoptosis system, adversary injection results, recovery time distributions.
5. **Evaluation methodology** — how to benchmark P2P learning runtimes, simulator validity, threats to validity.

---

## 15. Seek External Validation

**Level:** L4 (Research Differentiator) · **Effort:** Ongoing  
**Why:** Independent engagement is the strongest credibility signal. It proves someone besides the author cares.

**How:**
- Open source the repo, add `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`
- Share on relevant forums (Hacker News, /r/rust, /r/MachineLearning, P2P research mailing lists)
- Reach out to professors working on distributed learning, P2P systems, or neuromorphic computing
- Offer a reproducibility challenge: "Reproduce any figure in <5 minutes with one command"
- File issues labeled `good first issue` to attract contributors

---

## 16. Compare Against Established Systems

**Level:** L4 (Research Differentiator) · **Effort:** 2–4 weeks  
**Why:** Benchmarks without baselines are anecdotes. Show where NWP wins, loses, and makes different trade-offs.

**Baselines:**
- **Centralized:** Single-node with all peers in shared memory (ideal bandwidth, worst fault tolerance)
- **Gossip (standard):** Epidemic broadcast without DHT — every node talks to random peers
- **Random routing:** No structure — forward to random known peer
- **Static graph:** Pre-wired topology (grid, ring, tree)
- **Federated averaging:** Central aggregation server (FL-style)
- **Parameter server:** Pull-based parameter sync (standard ML distributed training)

**Already present:** None.

---

## 17. Measure Energy Efficiency

**Level:** L4 (Research Differentiator) · **Effort:** 1–2 weeks  
**Why:** Commodity hardware is part of the story. If NWP enables training on phone/laptop-class devices, energy is a first-order metric.

**What to measure:**
- CPU utilization per node (idle vs active)
- Memory usage per peer
- Network traffic and power implications
- Comparisons: NWP vs centralized training energy cost

---

## 18. Add Observability

**Level:** L4 (Research Differentiator) · **Effort:** 2–3 weeks  
**Why:** Makes debugging and experimentation much easier. Also impresses reviewers who have tried to debug distributed systems.

**What's needed:**
- **Distributed tracing:** Each packet carries a trace_id. Collect span data at each hop.
- **Metrics collection:** Prometheus endpoint exposing counters + histograms for every subsystem.
- **Structured logging:** JSON-formatted log lines with timestamps, node_id, span_id, event type.
- **Event replay:** Record all events to a ring buffer. Replay from any point in time.
- **Timeline visualization:** Time-series view of node state, packet flow, routing changes.

---

## 19. Package It Like Professional Software

**Level:** L3 (Engineering Excellence) · **Effort:** 1 week  
**Why:** First impressions matter. If the install is `cargo install neuron-wire` and it just works, that's a signal of quality.

**What's needed:**
- `README.md` with installation, quickstart, and examples (already decent)
- Versioned releases with semantic versioning (already tagged `v0.3.0-negative-sga`)
- `CHANGELOG.md` (missing)
- Installation guides for Linux, macOS, Windows (missing)
- API docs published to docs.rs or GitHub Pages (missing, but `cargo doc` works locally)
- `CONTRIBUTING.md` (missing)
- `LICENSE` (missing — critical)

---

## 20. Discover Something New

**Level:** L5 (Exceptional) · **Effort:** Ongoing  
**Why:** The project becomes research when it answers a question nobody already knows the answer to.

**Candidates already visible:**
1. **Maintenance pings contribute zero to routing quality** in stable local networks (SGA benchmark). This is counterintuitive — conventional DHT wisdom says periodic maintenance is essential. Evidence says otherwise.
2. **Does SGA's freshness guarantee matter more than bandwidth cost?** The trade-off between predictable failure detection and bandwidth overhead hasn't been characterized for P2P learning runtimes.
3. **At what churn rate does SGA become strictly better than fixed?** Fixed mode detects failures after `STALE_PING_S` seconds. SGA detects them within a freshness interval. The crossover point is a research question.
4. **Does the runtime converge under real WAN conditions?** The simulator assumes zero packet loss and symmetric routing. Real deployment will produce surprising failures.

**The most impactful single experiment:** Deploy to 3 continents, measure convergence time, compare to simulator prediction. Publish the delta — "Simulator predicted X but real world did Y, here's why." That's research.

---

## Priority Matrix

| # | Direction | Impact | Effort | Do First? |
|---|-----------|--------|--------|-----------|
| 1 | Distributed testbed | 🔥🔥🔥 | 2–3w | ✅ |
| 2 | Reproducibility package | 🔥🔥🔥 | 1w | ✅ |
| 3 | Formal protocol spec | 🔥🔥 | 1–2w | |
| 4 | Mathematical model | 🔥🔥🔥 | 2–3w | |
| 5 | Large benchmarks | 🔥🔥🔥 | 3–4w | |
| 6 | Interactive visualizer | 🔥🔥 | 2–3w | |
| 7 | Failure injection | 🔥🔥 | 1–2w | ✅ (mostly done) |
| 8 | Security | 🔥 | 2–4w | |
| 9 | Benchmark dashboard | 🔥🔥 | 1–2w | |
| 10 | Open protocol | 🔥🔥 | 1–2w | |
| 11 | Multiple impls | 🔥🔥 | 2–4w | |
| 12 | Publish datasets | 🔥 | 1w | |
| 13 | Research website | 🔥🔥 | 2–3w | |
| 14 | Multiple papers | 🔥🔥🔥 | Ongoing | |
| 15 | External validation | 🔥🔥🔥 | Ongoing | |
| 16 | Compare baselines | 🔥🔥 | 2–4w | |
| 17 | Energy efficiency | 🔥 | 1–2w | |
| 18 | Observability | 🔥🔥 | 2–3w | |
| 19 | Software packaging | 🔥🔥 | 1w | ✅ (low effort, high polish) |
| 20 | Discover something new | 🔥🔥🔥 | Ongoing | Always |

**Immediate next actions (this week):**
1. Add `LICENSE` (MIT/Apache 2.0) — 5 minutes, unblocks publishing
2. Write `CHANGELOG.md` — capture v0.1.0 through v0.3.0
3. Create `experiments/` directory with `figure4.toml` matching the SGA benchmark
4. Write `reproduce.sh` that runs all experiments and compares against known-good
5. Deploy 3 VPS seed nodes for distributed testbed
