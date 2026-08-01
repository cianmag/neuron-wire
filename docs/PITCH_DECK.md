# Neuron Wire Protocol — Pitch Deck

**Zero-infrastructure AI: one auditable P2P protocol for decentralized discovery, secure transport, and distributed learning.**

Zylvon · v0.3.1 · Rust · MIT · [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)

> Use with any slide tool. Each `## Slide N` = one slide; keep the narrative tight.
> Companion docs: GRANT_SUMMARY.md · EVIDENCE_REPORT.md · GRANT_BUDGET.md · GRANT_ROADMAP.md

---

## Slide 1 — Title
**Neuron Wire Protocol (NWP)**
Zero-infrastructure AI for free devices.
A single auditable P2P protocol: discovery + secure transport + distributed learning.

*Zylvon — validated research prototype, seeking $20,000 for the first real distributed testbed.*

---

## Slide 2 — The Problem
Collaborative AI is centralized by default.

- Parameter servers: single point of failure AND control — operator sees every gradient.
- Federated learning: needs a central coordinator; static topology.
- All-Reduce: fixed participant list; no discovery; no churn tolerance.
- Result: a device with a network connection cannot join AI training on its own terms.

**The gap:** no open infrastructure combines P2P discovery, secure transport, distributed
learning, and reproducible experimentation in one auditable codebase.

---

## Slide 3 — The Approach
One Rust protocol, four layers:

| Layer | What it does |
|---|---|
| **Discovery** | Latency-weighted Kademlia DHT — O(log N) convergence, ~constant bandwidth per node |
| **Transport** | Authenticated UDP: Ed25519 signatures, optional XChaCha20-Poly1305 encryption, replay protection, experimental X25519 key-exchange path |
| **Trust** | Trust-weighted rate limiting with proven bounded-score convergence — Sybil identities can't out-shout honest peers |
| **Learning** | Distributed Hebbian/STDP with gradient aging over a sparse P2P mesh |

Single-threaded engine, ~1 ms ticks, zero busy-wait. Targets 512 MB VPSs and low-power devices.

---

## Slide 4 — Why It's Novel
1. **Integrated stack** — discovery + security + learning + reproducibility harness in one codebase.
2. **Trust-weighted rate limiting** with convergence proofs (formal model, 1,760 lines).
3. **Deterministic paper-mode simulator** — failure injection (node death, partitions, malicious peers); every experiment reproducible with fixed seeds.
4. **Scaling evidence** — 100k-node DHT convergence at 100% in simulation, O(log N) time.

---

## Slide 5 — Validation Status (honest)
**Validated research prototype** — not a claimed deployed network.

✅ Deterministic simulation (10–500 nodes, fixed seeds)
✅ Local multi-process networking (2–25 real UDP processes)
✅ Network impairment emulation (tc netem: 20–300 ms, 0–10% loss; partition; attack)
✅ Property + fuzz testing; reproducible benchmarks
✅ Green CI on Linux/macOS/Windows + Docker + reproducibility check

⏳ Not yet: geographically distributed deployment · external security audit · independent replication

**That is exactly what funding unlocks.**

---

## Slide 6 — Evidence Pipeline (live)
Every push to the repo runs the full experiment matrix on CI and archives raw data:

```
E1  Convergence scaling: 10/25/50/100/500 nodes × seeds 42/1337/9001
E2  Node churn: 10/20/50% death at t=30s → recovery
E4  Packet loss: 2/5/10% deterministic in-sim impairment
E5  Malicious peer injection → trust response
E6  Network partition + recovery
E9  Baseline ablations: no-trust / no-aging / no-apoptosis / no-neurogenesis /
    random-discovery / static-topology vs control
```

**Evidence catches real bugs.** The first E1 run found the per-IP DoS guard
throttling localhost sims to ~10 peers (all nodes share 127.0.0.1) — fixed and
documented as a negative result. That is the pipeline working.

---

## Slide 7 — Baseline Comparisons (why NWP matters)
Without baselines, every result is "our system did a number."

| Ablation | What it isolates |
|---|---|
| Control (full stack) | NWP end-to-end |
| No trust scoring | Value of Sybil defense |
| No gradient aging | Value of aging for convergence |
| No apoptosis / no neurogenesis | Value of the brain lifecycle |
| Random discovery | Value of XOR-closest Kademlia |
| Static topology | Value of DHT maintenance |

Measured per ablation: convergence time, avg peers, bytes, packets, bandwidth, recovery.

---

## Slide 8 — Funding: $20,000
Funding converts a validated simulation + local prototype into a **geographically
distributed research testbed** — not salaries.

| Category | Amount |
|---|---|
| Cloud test infrastructure | $4,000 |
| Low-power devices + networking hardware | $3,000 |
| Security review | $5,000 |
| Research compute + storage | $3,000 |
| Domain, monitoring, services | $1,000 |
| Documentation + publication | $1,000 |
| Contingency | $3,000 |
| **Total** | **$20,000** |

---

## Slide 9 — Funded Milestones
| # | Milestone | Duration | Cost |
|---|---|---|---|
| M1 | Green CI + verified alpha | 1 wk | $1,000 |
| M2 | Reproducible 100-node simulation | 2 wk | $500 |
| M3 | Local 25-process test | 2 wk | $1,000 |
| M4 | Network emulation benchmark | 3 wk | $2,500 |
| M5 | **20-node, 3-region distributed pilot** | 6 wk | $8,000 |
| M6 | Public dataset + research preprint | 4 wk | $2,500 |
| M7 | External reproduction | 4 wk | $1,500 |

Every milestone has measurable acceptance criteria (see GRANT_ROADMAP.md).

---

## Slide 10 — The Founder
Built entirely without institutional support: a ~21,000-line Rust codebase, formal
model with convergence proofs, CI-gated validation, reproducible evidence pipeline,
and a grant package — on one laptop and free CI runners.

**The message is not "young, therefore fund me."**
It is: *substantial technical evidence produced with limited resources; funding
crosses the boundary from prototype to real-world research infrastructure.*

---

## Slide 11 — Ask & Next Steps
**The ask:** $20,000 for a funded 20-node, 3-region distributed pilot (M5) plus
external security review and publication.

**What you get:**
- A working open-source protocol + public dataset + preprint
- Measurable milestones with acceptance criteria
- An honest limitations section — no inflated claims

**Contact:** via GitHub [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)

*"We produced substantial technical evidence with limited resources. Funding will
let the project cross the boundary from prototype to real-world research infrastructure."*
