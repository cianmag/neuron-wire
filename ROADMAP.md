# Research Roadmap: From Simulator to Funded Infrastructure Platform

> **Date:** 2026-07-03  
> **Repository:** [neuron-wire](https://github.com/cianmag/neuron-wire)  
> **Status:** Aligned with Zylvon infrastructure roadmap (see [GRANT.md](GRANT.md))

This document maps the three deliverables that would transform neuron-wire from a sophisticated localhost prototype into a funded, published, and adopted decentralized AI infrastructure platform.

[See GRANT.md](GRANT.md) for the full grant pitch with budget, timeline, and success criteria.

---

## Deliverable 1 — Real Internet Deployment (100 Nodes, 3 Continents)

Take neuron-wire from localhost to the real Internet. Deploy across free-tier VPS in North America, Europe, and Asia. Measure what actually happens.

### Why This Matters

Every benchmark so far is localhost UDP. Real networks have NAT, jitter, loss, asymmetric routing, and heterogeneous hardware. We don't know if any of our assumptions survive contact with the real Internet. This is the first real-world test of the architecture.

### Tasks

- [ ] Deploy 100+ nodes across AWS free-tier (us-east-1, eu-west-1, ap-southeast-2)
- [ ] Measure: latency distribution (intra/inter-region), packet loss, DHT convergence time, gradient delivery rate
- [ ] Build public dashboard showing per-region live metrics
- [ ] Implement Noise Protocol encryption (DTLS-style, zero-alloc)
- [ ] Implement STUN-based NAT traversal for non-VPS nodes
- [ ] Publish negative results alongside positive ones

### Success Criteria

- 100+ nodes sustain connectivity for 7+ days
- Cross-continent gradient delivery with < 50% loss
- Public dashboard operational

---

## Deliverable 2 — Publication & Reproducible Benchmarks

### Why This Matters

No decentralized AI infrastructure project has published a reproducible evaluation with open datasets. The field is full of claims and empty of evidence. Fixing this would be a contribution regardless of whether our architecture wins or loses.

### Tasks

- [ ] Write evaluation paper comparing NWP against: vanilla Kademlia, gossip SGD, federated averaging
- [ ] Run all 7 Python baselines on the same testbed with same measurement methodology
- [ ] Generate open datasets (per-tick latency, throughput, convergence curves for all 4 systems × 3 topologies)
- [ ] Submit to workshop (SysML, MLSys, HotOS, DSN) — registered report format preferred
- [ ] `cargo run --release -- --reproduce-all` reproduces every figure

### Success Criteria

- Paper accepted at workshop or conference
- Datasets under CC-BY license
- Figures one-command reproducible from fresh clone

---

## Deliverable 3 — Developer SDK & Ecosystem

### Why This Matters

Neuron Wire is 19,000 lines of Rust with no onboarding path. The fastest way to use it is to clone the repo and read examples. This limits adoption to other Rust systems programmers.

### Tasks

- [ ] Publish to crates.io (`cargo add neuron-wire`)
- [ ] Minimal public API: `start_node`, `connect`, `send_gradient`, `recv_gradient`, `get_stats`
- [ ] Python bindings via PyO3 (import from Python, use with numpy)
- [ ] Document 5 reference applications: decentralized LLM inference, collaborative edge learning, disaster communication AI, decentralized robotics, distributed scientific computing
- [ ] Peer cache persistence and snapshot/restore
- [ ] WebRTC transport for browser-to-browser across machines

### Success Criteria

- `cargo add neuron-wire` → running node in 5 minutes
- Python wheel on PyPI
- At least 2 external contributors from other research groups

---

## Pre-Deliverable: What Already Exists

Before the grant deliverables begin, the following is already in place:

### Core Infrastructure (19,220 lines of Rust, 42 modules)

| Component | Status | Evidence |
|-----------|--------|----------|
| Peer discovery (Kademlia DHT) | ✅ | 1,209 lines, 7 unit tests, 256 K-buckets, K=20, DNS seed bootstrap |
| Secure transport (UDP) | ✅ | 642 lines, 10 tests, 3 reliability tiers, ACK bitfield, gradient decay |
| Engine loop | ✅ | 1,045 lines, 6-phase tick, ~400 KHz–1 MHz, 0% CPU when idle |
| Forward pass + Hebbian STDP | ✅ | 413 + 683 lines, 15 tests, tanh propagation, STDP, micro-pruning, gossip |
| Neurogenesis + Apoptosis | ✅ | 340 + 306 lines, 13 tests, surprise-driven birth, 4-criteria death |
| Identity (Ed25519) | ✅ | 339 lines, full key lifecycle |
| Trust scoring | ✅ | 504 lines, Sybil-resistant |
| Audit log | ✅ | 534 lines, hash-chained |
| Observability | ✅ | Prometheus, OpenTelemetry, live dashboard |
| Simulator (paper-mode) | ✅ | 1,515 lines, metadata capture, CSV export, known-good validation |

### Research Infrastructure

| Asset | Status | Details |
|-------|--------|---------|
| Formal mathematical model | ✅ | 1,760 lines, 17 sections: convergence proofs, complexity bounds, failure probabilities |
| Baseline comparisons | ✅ | 7 Python frameworks (federated, decentralized SGD, parameter server, Horovod, BitTensor, Ray, comparison harness) |
| Reproducible experiments | ✅ | 10 TOML configs, one-command reproduce.sh, 9 matplotlib figures |
| Architecture decisions | ✅ | 8 ADRs with rationale |
| Tutorial series | ✅ | 6 tutorials covering first node to deep observability |
| Browser demo (WASM) | ✅ | Pure WASM P2P neural network, two tabs, no server |
| CI/CD | ✅ | 4 workflows (build+test, docs, release, reproduction) |
| Community files | ✅ | CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, 5 issue templates |

### What Requires Grant Funding

| Gap | Why We Can't Close It Alone | Which Deliverable |
|-----|----------------------------|-------------------|
| Real Internet deployment (VPS across 3 continents) | Compute costs for 100+ nodes × 3 months | D1 |
| NAT traversal engineering | Requires protocol-level changes and Internet-scale testing | D1 |
| Noise Protocol encryption | Crypto protocol implementation + audit | D1 |
| Publication fees + open-access costs | Venue fees, potential travel | D2 |
| Baseline evaluation on real testbed | Requires D1 deployment to be meaningful | D1 + D2 |
| SDK polish + crates.io publishing | API stabilization and documentation effort | D3 |
| Python bindings (PyO3) | Specialized engineering effort | D3 |
| WebRTC transport | Protocol-level implementation | D3 |
| Community building + documentation | Time investment with no immediate engineering output | D3 |

---

## Timeline (Estimated)

| Quarter | Deliverable | Milestone |
|---------|-------------|-----------|
| Q3 2026 | D1: Real Internet deployment | 100+ nodes across 3 continents, public dashboard |
| Q3–Q4 2026 | D2: Publication | Paper submission, open dataset release |
| Q4 2026–Q1 2027 | D3: SDK + ecosystem | crates.io, PyPI, 5 reference apps |
| Q1 2027 | All deliverables | Paper decision, community metrics review |

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Real Internet reveals fundamental protocol flaw | Medium | Publish negative results — still a contribution to know what doesn't work |
| Low adoption despite SDK investment | Medium | Target 2 specific external contributors rather than mass adoption; quality over quantity |
| Grant not awarded | N/A | Continue as unfunded research; deliverables remain as roadmap |
| NAT traversal infeasible for some deployment patterns | Medium | Document constraints, provide STUN/TURN fallback options |
| Paper rejected | Medium | Multiple venue pipeline (workshop → conference → journal); negative results also publishable |
