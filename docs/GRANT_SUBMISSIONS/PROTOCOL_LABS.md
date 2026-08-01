# Protocol Labs Research Grant — Application Draft

**Status:** DRAFT for submission · verified 2026-08-01 · research.protocol.ai/outreach/grants (active)
**Applicant:** Zylvon (solo founder, 16, Nepali citizen studying in India — guardian consent available)
**Project:** Neuron Wire (NWP) — validated P2P infrastructure for decentralized AI
**Request:** $20,000 · **Timeline:** 12 months

---

## Title
**Neuron Wire: A validated, reproducible infrastructure layer for decentralized neural-gradient exchange over peer-to-peer networks**

## One-paragraph summary
Neuron Wire (NWP) is an MIT-licensed Rust implementation of the infrastructure layer for planetary-scale decentralized AI: nodes discover each other over a latency-weighted Kademlia DHT, exchange neural gradients over a custom reliable-UDP transport, and learn collectively with Hebbian STDP — no central coordinator, no data leaving the device, no async-runtime dependency. The prototype is validated (342 tests on Linux CI, E1–E9 experiment matrix, deterministic simulation to 1 B nodes, real local UDP multi-process tests, network-impairment emulation, adversarial testing, and a seed-reproducible end-to-end distributed-learning test over real sockets). Funding is requested to cross the boundary from prototype to research infrastructure: a geographically distributed pilot, independent reproduction, and comparison against established baselines.

## Problem
Centralized training concentrates power, trust, and cost in a single provider; decentralized alternatives (FedAvg, decentralized SGD, gossip learning) have been studied in simulation but rarely validated as *real network systems* with honest operational measurements. The open problems are operational: how do latency-weighted DHT routing, gradient aging, and trust-based rate limiting behave on the real Internet, under churn, partition, and adversarial conditions — reproducibly?

## Prior work / related work
- Kademlia (Maymounkov & Mazières, 2002) — base routing; NWP adds latency weighting and trust-aware neighbor selection.
- Decentralized parallel SGD (Lian et al., 2017) — NWP's learning baselines; we will compare directly (Milestone 5).
- FedAvg (McMahan et al., 2017) — central-server baseline for comparison.
- IPFS/libp2p (Protocol Labs ecosystem) — complementary; NWP targets single-threaded no-async-runtime UDP for commodity/edge hardware, and is designed to interop at the gradient-payload layer.

## What exists today (all reproducible)
- **342 test functions pass** on Linux CI (358 run, 16 ignored), zero-warning clippy, 19 benchmarks, 4 fuzz targets — commit-pinned in `docs/EVIDENCE_REPORT.md` with direct CI/evidence-run links.
- **E1–E9 matrix**: node churn, partition tolerance, malicious routing, gradient decay, trust ablation, neurogenesis (robust +23–25% bandwidth effect), and an honest negative result (sparse-gradient ablation, published as such).
- **Deterministic simulations**: 100 K nodes converge 100% in 43 s; hybrid statistical model scales to 1 B nodes.
- **Real sockets**: 25-process localhost cluster; E2E distributed learning (A fires activation → B's synapse 0.5 → >0.6 → signal returns; same seed, same result).
- **Adversarial testing**: corruption, churn, identity-spoof, replay vectors.
- **7 pipeline-caught bugs** documented as negative results (incl. an Adam-optimiser NaN bug caught by the learning E2E).
- Commit `265e216` (v0.3.1), evidence run green, tag `v0.3.1`.

## Proposed work (12 months)
| # | Milestone | Deliverable | Success metric |
|---|-----------|-------------|----------------|
| M1 | **Secure distributed-learning E2E** | `tests/secure_distributed_learning_e2e.rs` — deterministic Ed25519 identities, signed-frame acceptance, unsigned/modified-frame rejection, encrypted payload transport, replay rejection, synapse update only after authentication, authenticated gossip response | All green on CI |
| M2 | **20-node, three-region pilot** (NA/EU/APAC) | Cloud-deployed mesh, public metrics dashboard | 7-day sustained mesh; <50% cross-region gradient loss |
| M3 | **NAT traversal** | STUN-backed hole punching on the UDP transport | ≥3 real asymmetric-NAT pairs connect |
| M4 | **Seven-day soak** | Continuous churn/partition injection during pilot | Connectivity ≥99%; convergence <30 s after partition events |
| M5 | **Baseline comparison** | FedAvg + decentralized SGD on identical gradient workloads | Published comparison dataset; paper |
| M6 | **Publication + reproduction** | Reproducible datasets, paper including negative findings | One-command reproduction from fresh clone |

## Budget ($20,000)
| Category | Amount |
|----------|--------|
| Cloud compute (3 regions × 7-day pilot + soak) | $4,000 |
| Hardware (edge-node testbed for NAT/latency realism) | $3,000 |
| External security review of cryptographic + transport layers | $5,000 |
| Research compute (baseline training, reproduction runs) | $3,000 |
| Domain, monitoring, CI beyond free tier | $1,000 |
| Documentation, dataset packaging | $1,000 |
| Contingency (15%) | $3,000 |
| **Total** | **$20,000** |

## Why this is a good fit for Protocol Labs
- Pure P2P / decentralized-computing research — directly in the research grant mandate.
- Reproducible-from-clone evidence is the norm here, not the exception.
- Interop potential with libp2p at the gradient-payload layer; both communities benefit from a validated UDP-first alternative.

## Team & execution
Solo founder who designed and built the entire system (Rust, ~21,000 lines, 41 modules) with zero funding, on free CI. Working artifacts and one-command reproduction of every experiment are already public. Guardian consent available for any legal/financial steps. Execution plan assumes a 10–15 h/week research schedule alongside secondary education; the plan's milestones are sized accordingly.

## Sustainability & open source
MIT-licensed from day one. All datasets, scripts, and the evidence pipeline are public. After the pilot, the project's sustainability path is: (a) the decentralized-deployment thesis itself (nodes are free to run), (b) SDK/ecosystem revenue later (crates.io/PyPI), (c) continued grant cycles (Rust Foundation, NGI/OIS post-summer 2026).

## Contact
[Founder name + email] · github.com/cianmag/neuron-wire · grant package in `docs/GRANT_*`, technical summary in `docs/TECHNICAL_SUMMARY.md`, full evidence in `docs/EVIDENCE_REPORT.md`
