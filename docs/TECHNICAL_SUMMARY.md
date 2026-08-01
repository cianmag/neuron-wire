# Neuron Wire — Technical Summary

**Version v0.3.1 · Validated Research Prototype · MIT License**
Zylvon · [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) · evidence pinned in
[docs/EVIDENCE_REPORT.md](docs/EVIDENCE_REPORT.md)

---

## 1. What it is

**Neuron Wire (NWP)** is a Rust framework that lets any device reachable over a network participate in
collaborative AI — without a central coordinator, without provisioning servers, and without exposing
private data. It is the infrastructure layer for a planetary-scale, peer-to-peer neural network: nodes
discover each other, exchange neural gradients, and learn collectively.

The core is a **single-threaded, non-blocking UDP engine** running at ~400 KHz–1 MHz ticks on commodity
hardware, with **no async runtime dependency**. The protocol combines:

| Subsystem | What it does | Status |
|-----------|--------------|--------|
| Latency-weighted Kademlia DHT | Peer discovery; 256 buckets, K=20; converges in O(log N) | ✅ validated |
| Reliable UDP transport | 3 reliability tiers, ACK bitfield, gradient aging | ✅ validated |
| Hebbian STDP learning | Spike-timing-dependent plasticity, weight decay, sparse gossip | ✅ validated (engine) |
| Ed25519 identity + AEAD | Packet authentication; optional XChaCha20-Poly1305 encryption | ✅ validated (subsystem) |
| Trust-based rate limiting | Sybil resistance, per-IP DoS guards | ✅ validated |
| Deterministic simulator | Paper-mode simulation with fixed seeds, known-good validation | ✅ validated |

## 2. Evidence (commit-pinned, reproducible)

Every number below is produced by **one verified commit** and **one CI/evidence run**, linked from
`docs/EVIDENCE_REPORT.md` — reproducible from a fresh clone in minutes.

- **342 test functions pass** on Linux CI (358 run, 16 ignored), zero-warning clippy, 19 benchmarks, 4 fuzz targets.
- **E1–E9 experiment matrix**: node churn, partition tolerance, malicious routing, gradient decay, trust
  ablation, neurogenesis (robust **+23–25%** bandwidth effect), sparse-gradient negative result (published as such).
- **Deterministic simulations**: 100 K nodes converge 100% in 43 s; hybrid model scales to 1 B nodes.
- **Real local UDP**: 25-process localhost cluster converges; multi-process E2E with real sockets.
- **Network emulation** (`tc netem`): 4 impairment scenarios vs. baselines, raw data archived per run.
- **Adversarial testing**: corruption, state-churn, identity-spoof, replay-attack vectors.
- **End-to-end distributed learning**: a real UDP test where node A sends an activation, node B's synapse
  updates (0.5 → >0.6) and returns a learning signal — seed-reproducible. This test caught a real
  production bug (Adam optimiser NaN on first tick), now fixed with regression coverage.
- **7 pipeline-caught bugs** documented as negative results — a limitation documented is stronger than an
  assumption hidden.

## 3. Provenance

| Field | Value |
|-------|-------|
| Version | v0.3.1 |
| Pinned commit | `265e2169949c180d7ad3a0ce0dcf73c4b75687b7` |
| CI run | [ci.yml 30697662079](https://github.com/cianmag/neuron-wire/actions/runs/30697662079) — green |
| Evidence run | [evidence.yml 30697662078](https://github.com/cianmag/neuron-wire/actions/runs/30697662078) — matrix + emulation, green |
| Test count | 342 passing on Linux CI |

Documentation-only commits after the pinned commit do not alter measured results; the tag's own CI
re-verifies this.

## 4. Funding request — $20,000

The prototype is validated; what is unproven is *operational* behavior on the real Internet. Funding
buys the boundary crossing from prototype to research infrastructure:

1. **20-node, three-region pilot** (NA/EU/APAC), sustained 7 days, public metrics dashboard — the step
   simulation cannot take. Scaling beyond the pilot is driven by its data.
2. **External security audit** of the cryptographic and transport layers.
3. **Independent reproduction** + comparison against FedAvg and decentralized SGD baselines.
4. **Publication** — reproducible datasets, baselines, and a peer-reviewed paper including negative findings.

Budget: $4,000 cloud · $3,000 hardware · $5,000 security review · $3,000 research compute · $1,000
domain/monitoring · $1,000 documentation · $3,000 contingency. $10k / $5k alternative packages exist.

## 5. What is honestly not yet true

- No NAT traversal (all nodes must be directly reachable on UDP).
- Encryption is optional and off by default until the ECDH handshake is hardened.
- Combined *encrypted-and-authenticated* distributed-learning E2E is not yet demonstrated (security and
  learning paths are each tested; the combined test is the next milestone).
- No BFT consensus (>⅓ malicious routing-table nodes can partition).
- No persistent storage, single-threaded engine, DHT-only discovery.

## 6. Roadmap

| Milestone | Deliverable | Acceptance |
|-----------|-------------|------------|
| M1 | Green CI + verified alpha | ✅ done — 342 tests, commit-pinned |
| M2 | Reproducible 100-node simulation | ✅ done |
| M3 | Local 25-process test | ✅ done |
| M4 | Network emulation benchmark | ✅ done |
| M5 | **Funded 20-node, three-region pilot** | 7-day mesh, public dashboard — *funding required* |
| M6 | Publication + audit | Reproducible paper, external security review — *funding required* |
| M7 | SDK + ecosystem | crates.io, PyPI, 5 reference apps — *post-pilot* |

---

*Full detail: [EVIDENCE_REPORT.md](docs/EVIDENCE_REPORT.md) · [GRANT_SUMMARY.md](docs/GRANT_SUMMARY.md) ·
[GRANT_BUDGET.md](docs/GRANT_BUDGET.md) · [GRANT_ROADMAP.md](docs/GRANT_ROADMAP.md) ·
[GRANT_RISKS.md](docs/GRANT_RISKS.md) · [GRANT_PREPRINT.md](docs/GRANT_PREPRINT.md)*
