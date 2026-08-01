# NLnet — Application Draft (for Open Internet Stack reopening)

**Status:** PREPARED for the post-summer 2026 reopening · verified 2026-08-01
NLnet paused open calls 2026-06-12 (NGI → Open Internet Stack transition; "after the summer the
regular application process will re-open with 3 new programs"). Commons Fund final call closed
2026-06-01. **Action: re-verify nlnet.nl/apply for the OIS call dates; submit within 2 weeks of
reopening.** Minors OK with guardian consent for negotiations.

---

## Project name
Neuron Wire (NWP)

## Tagline
Infrastructure for decentralized AI: peer-to-peer discovery, gradient exchange, and distributed learning — no central coordinator, no async runtime, no data leaving the device.

## Abstract (~150 words)
Neuron Wire is an MIT-licensed Rust framework that turns any reachable device into a participant in collaborative machine learning. Nodes discover each other through a latency-weighted Kademlia DHT, exchange neural gradients over a custom reliable-UDP transport with gradient aging, and learn collectively with Hebbian STDP — with Ed25519 authentication, optional AEAD encryption, and trust-based rate limiting. The system is validated, not aspirational: 342 tests pass on Linux CI (zero-warning clippy, 19 benchmarks, 4 fuzz targets), an E1–E9 experiment matrix covers churn, partition, and adversarial conditions, deterministic simulation scales to 100 K nodes (100% convergence) with a hybrid model to 1 B, real local UDP multi-process tests converge, network-impairment emulation is archived per run, and a seed-reproducible end-to-end distributed-learning test runs over real sockets. Every number is pinned to one commit and one CI/evidence run. Funding is requested for the step simulation cannot take: a 20-node, three-region pilot with independent reproduction, security review, and baseline comparison.

## Problem
Centralized AI concentrates power, trust, and cost. Decentralized alternatives exist in theory and simulation, but almost no system has published *reproducible operational* measurements of decentralized gradient exchange on the real Internet. The public-good question is whether open P2P infrastructure can deliver collaborative learning without a trusted center — and how it behaves under churn, partition, and malice, honestly measured.

## Solution
NWP's answer, already implemented and validated: latency-weighted Kademlia routing; reliable UDP with three reliability tiers and exponential gradient aging (bounded staleness); Hebbian STDP learning with neurogenesis/apoptosis; Ed25519 packet authentication with optional XChaCha20-Poly1305; trust-based rate limiting; deterministic paper-mode simulation. A single-threaded non-blocking engine (~400 KHz–1 MHz ticks, no async runtime) keeps the node runnable on commodity hardware — the "free devices as neural net" thesis.

## Impact
- **Digital commons**: an open, reproducible alternative to centralized training infrastructure.
- **Edge inclusion**: devices too small/constrained for centralized stacks can participate.
- **Research infrastructure**: first reproducible WAN benchmark suite for P2P gradient exchange (latency-characterized STDP convergence, churn tolerance, adversarial behavior) — datasets and scripts public under open license.
- **Community**: MIT license, zero-dependency core, design documents, and a public dashboard from day one.

## Work plan (12 months)
| Months | Deliverable | Success metric |
|--------|-------------|----------------|
| 1–2 | Secure distributed-learning E2E test (auth + encryption + replay rejection) | Green on CI |
| 3–4 | 20-node, three-region pilot deployment (NA/EU/APAC) + public dashboard | Mesh sustained |
| 5–6 | Seven-day soak with churn/partition injection | ≥99% connectivity; <30 s reconvergence |
| 7–8 | NAT traversal (STUN hole punching) on the UDP transport | ≥3 asymmetric-NAT pairs connect |
| 9–10 | Baseline comparison: FedAvg + decentralized SGD | Published dataset |
| 11–12 | Paper + independent reproduction from fresh clone | One-command reproduction |

## Budget (€18,500 ≈ $20,000)
| Category | EUR |
|----------|-----|
| Cloud compute (3 regions × pilot + soak) | €3,700 |
| Hardware (edge testbed) | €2,800 |
| External security review | €4,600 |
| Research compute (baselines, reproduction) | €2,800 |
| Domain, monitoring, CI | €900 |
| Documentation, dataset packaging | €900 |
| Contingency (15%) | €2,800 |
| **Total** | **€18,500** |

## Team
Solo founder (16, Nepali citizen studying in India — guardian consent available for all negotiations), who designed and built the entire system: ~21,000 lines of Rust across 41 modules, 342 tests, formal model, wire-protocol spec, and a 7-negative-result bug log — all public. Execution is sized to a 10–15 h/week research schedule; the milestones above are deliberately small enough to be verifiable at each step.

## Sustainability
MIT from day one; the deployment thesis is self-sustaining (nodes are free to run; the network is the infrastructure). Post-pilot: SDK/ecosystem (crates.io/PyPI), further grant cycles (Rust Foundation, Protocol Labs), and a university-partner path for org-gated funding. All datasets remain public regardless of funding outcome.

## Contact
[Founder name + email] · github.com/cianmag/neuron-wire · docs/EVIDENCE_REPORT.md (commit-pinned evidence) · docs/GRANT_* (full package) · docs/TECHNICAL_SUMMARY.md
