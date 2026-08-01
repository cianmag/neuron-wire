# Neuron Wire Protocol (NWP) — Grant Summary

**Zero-infrastructure AI: one auditable P2P protocol for decentralized discovery, secure transport, and distributed learning.**
Zylvon · [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) · v0.3.1 · Rust · MIT

## Problem
Collaborative AI is centralized by default. Gradients flow through parameter servers; federated learning still needs an aggregator; All-Reduce assumes a static participant set. Every participant must trust a central operator — for availability, privacy, and honesty — and a device with a network connection cannot join on its own terms. No open infrastructure combines P2P discovery, secure transport, distributed learning, and reproducible experimentation in a single auditable codebase.

## Current systems and their limitations
| System | Limitation |
|---|---|
| Parameter-server training | Central point of failure and control; operator sees all gradients |
| Federated learning | Central coordinator; server-mediated rounds; static topology |
| All-Reduce (Horovod) | Static known peers; no discovery, no churn tolerance |
| Gossip-SGD overlays | Assume an existing membership layer; no identity or Sybil defense |

## Neuron Wire approach
One Rust protocol combining every layer: latency-weighted Kademlia DHT discovery; authenticated UDP transport (Ed25519 signatures + optional XChaCha20-Poly1305 encryption, replay protection, and an experimental X25519 key-exchange path); trust-based rate limiting with proven convergence (Sybil resistance); and distributed Hebbian/STDP learning with gradient aging over a sparse P2P mesh. Single-threaded engine, ~1 ms ticks, zero busy-wait, targeting 512 MB VPSs and low-power devices.

## Technical novelty
- **Integrated stack** — discovery + secure transport + distributed learning + reproducibility harness in one codebase, backed by a 1,760-line formal model with convergence theorems.
- **Trust-weighted rate limiting** with bounded-score convergence proofs — Sybil identities cannot out-shout honest peers.
- **Deterministic paper-mode simulator** with failure injection (node death, partitions, malicious peers) — experiments are reproducible, not anecdotal.
- **Scaling evidence** — 100k-node DHT convergence at 100% in simulation, O(log N) convergence time, ~constant bandwidth per node; 1B-node hybrid model as a theoretical bound.

## Current status — validated research prototype
| Dimension | Evidence |
|---|---|
| Codebase | 41 modules · 20,900+ lines Rust · 342 tests passing on Linux CI · 4 CI workflows · zero-warning clippy |
| Correctness | Unit/integration/property tests + 4 fuzz targets; 100% pass on supported toolchains |
| Simulation | Deterministic paper-mode simulator; 100k-node convergence; failure injection incl. malicious nodes |
| Local real UDP | Multi-process localhost tests; OS socket limits identified as the scaling constraint — not the protocol |
| Security | Per-packet Ed25519 auth, AEAD encryption, replay protection, hash-chained audit log, 28+ Prometheus metrics |

**Honest boundary:** validated via deterministic simulation, local multi-process tests, property/fuzz testing, and reproducible benchmarks. **Not yet:** geographically distributed deployment, external security audit, independent replication.

## Experimental results
Every run is deterministic (fixed seeds 42/1337/9001, `--paper-mode`), runs on CI per push,
and archives raw CSVs. Live table: `docs/EVIDENCE_REPORT.md` + `results/evidence/`.

| ID | Experiment | Status |
|---|---|---|
| E1 | Convergence scaling: 10/25/50/100/500 nodes × 3 seeds | ✅ pipeline live — post-fix numbers regenerating on CI |
| E2 | Node churn (10/20/50% death at t=30s, recovery) | ✅ pipeline live |
| E4 | Deterministic packet loss (2/5/10%, seeded in-sim) | ✅ pipeline live |
| E5 | Malicious peer injection (trust response) | ✅ pipeline live |
| E6 | Network partition + recovery | ✅ pipeline live |
| E9 | Baseline ablations: no-trust / no-aging / no-apoptosis / no-neurogenesis / random-discovery / static-topology vs control | ✅ pipeline live |
| — | Local multi-process: 2/5/10/25 real UDP processes | ✅ pipeline live |
| — | Network emulation: normal/mobile/weak/severe (netem) + partition + attack | ✅ pipeline live |
| M5 | 20-node, 3-region distributed pilot | funded phase |
| M7 | Security: Sybil, replay, eclipse — external review | funded phase |

## Funding request — $20,000
Funding will convert a validated simulation and local prototype into a **geographically distributed research testbed**: a 20-node, 3-region pilot with network-emulated and real-Internet measurements, an external security review, and a public dataset with a research preprint. The request covers infrastructure, hardware, external review, and publication — **not salaries**. Full breakdown in GRANT_BUDGET.md.

## Funded milestones (details in GRANT_ROADMAP.md)
| Milestone | Cost |
|---|---|
| M1 Green CI and verified alpha | $1,000 |
| M2 Reproducible 100-node simulation | $500 |
| M3 Local 25-process test | $1,000 |
| M4 Network emulation benchmark | $2,500 |
| M5 Funded 20-node distributed pilot | $8,000 |
| M6 Public dataset and research preprint | $2,500 |
| M7 External reproduction | $1,500 |
| Contingency | $3,000 |
| **Total** | **$20,000** |
