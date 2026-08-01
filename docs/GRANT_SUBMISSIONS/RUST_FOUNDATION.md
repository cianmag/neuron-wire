# Rust Foundation Community Grant — Application Draft

**Status:** ⚠️ **ELIGIBILITY-BLOCKED — draft ready, DO NOT SUBMIT** · verified 2026-08-01
**Blocking clause (official [Rust Foundation grants-eligibility policy](https://rustfoundation.org/policy/grants-eligibility/)):**
> "All grant applicants must be over the age of 18 by the date of the grant award. All grant
> applicants must ensure that they are able to receive financial transfers from the USA."

Applicant is 16 (turns 17 in October 2026) → **ineligible under the age clause today**, and the
US financial-transfer receipt clause must be verified for Nepal. See the eligibility note at the
bottom of this file — it is part of the draft, not a footnote.

**Applicant:** Zylvon (solo maintainer — age stated openly, never hidden)
**Project:** Neuron Wire (NWP) — synchronous no-async-runtime Rust P2P runtime
**Request:** $12,000 (Project Grant scale; adjust once the 2026 round's terms are verified)
**Submission status:** BLOCKED — pending (a) verified 2026 round + deadline, (b) an open age
pathway (official written confirmation, adult fiscal sponsor, or institutional applicant).

---

## Title
**Hardening and Reproducing a Synchronous Rust Runtime for Decentralized Learning**

## One-paragraph summary
Neuron Wire (NWP) is an MIT-licensed, synchronous, non-blocking Rust runtime for peer-to-peer
decentralized learning — no async runtime, no std-dependency claims, no framework. Over the last
year it has grown into a reproducible systems artifact: 342 tests pass on Linux CI (358 run,
16 ignored), a nine-experiment matrix (E1–E9) covering churn, partition, adversarial presence,
and ablations, real local multi-process UDP tests, network-impairment emulation, deterministic
simulation to 100,000 nodes, and a seed-reproducible end-to-end distributed-learning test over
real sockets. Every result is pinned to one commit (`265e216`, v0.3.1) with linked CI and
evidence runs. This grant funds the parts of that work the Rust ecosystem can reuse directly:
hardening the protocol-testing and fuzzing story for synchronous Rust networking, publishing a
cross-platform benchmark corpus, and documenting what it takes to build decentralized systems in
Rust without an async runtime.

## Problem — and why it is a Rust-ecosystem problem
Synchronous non-blocking networking in Rust is under-documented and under-tested relative to its
async counterparts. `tokio`/`async-std` dominate the narrative, yet a meaningful class of systems
(embedded, edge, deterministic simulators, single-threaded event engines) deliberately avoid
async runtimes for predictability and resource footprint. Practitioners rebuilding those systems
re-derive the same protocols, test vectors, and fuzz harnesses from scratch. NWP is a concrete,
working instance of that class — and it can be packaged so the ecosystem does not repeat the
work. The open problems this grant addresses are ecosystem problems: reliable-UDP behavior under
impairment, malformed-packet handling, cross-platform build parity, and honest profiling data for
synchronous Rust networking.

## Proposed work (12 months) — all deliverables public and reusable
| # | Milestone | Deliverable | Success metric | Ecosystem value |
|---|-----------|-------------|----------------|-----------------|
| M1 | **Secure distributed-learning E2E test** | `tests/secure_distributed_learning_e2e.rs`: deterministic Ed25519 identities, signed-frame acceptance, unsigned/modified-frame rejection, replay rejection, synapse update only after authentication | Green on Linux CI, commit-pinned | Reusable pattern for authenticated P2P learning tests |
| M2 | **Cross-platform benchmark corpus** | Benchmarks for the synchronous engine + transport on Linux x64/arm64, macOS x64/arm64, Windows x64 | Same-commit benchmark artifacts published; machine-readable results | Public profiling data for sync-Rust networking |
| M3 | **Fuzz + malformed-packet datasets** | Corpus of malformed/invalid NWP frames + fuzz targets with seeds | All fuzz targets green; corpus published | Reusable malformed-frame test data for UDP protocol authors |
| M4 | **Reusable protocol test vectors** | Deterministic vectors for header parse, gradient aging, trust scoring, replay ring buffer | Vectors published; consumed by any third-party implementation | Cross-language protocol conformance testing |
| M5 | **Public engineering report** | Post-mortem of 7 pipeline-caught bugs (incl. an Adam NaN) + design notes on the synchronous engine | Report published with reproductions | Honest failure-mode documentation for sync-Rust systems |
| M6 | **Independent reproduction** | Third party reproduces all benchmark + fuzz results from a fresh clone | Reproduction report accepted | Establishes the corpus as trustworthy ground truth |

## Budget ($12,000 — Project Grant scale; adjust to verified 2026 terms)
| Category | Amount |
|----------|--------|
| CI matrix (Linux/macOS/Windows benchmark + fuzz runs, 12 months) | $2,500 |
| Hardware (edge/ARM testbed for cross-platform parity) | $2,000 |
| Fuzzing + adversarial infrastructure (corpus growth, crash triage) | $2,500 |
| Documentation + engineering report production | $2,000 |
| Reproduction runs + artifact packaging | $1,500 |
| Contingency (12.5%) | $1,500 |
| **Total** | **$12,000** |

## Why this benefits the Rust ecosystem beyond Neuron Wire
- **Reusable reliable-UDP components** — the three-tier reliability transport, gradient-aging
  queue, and impairment emulation ship as documented, tested modules any Rust networking project
  can study or fork.
- **Fuzzing and protocol-security practices** — the corpus + harnesses demonstrate how to fuzz a
  binary UDP protocol without an async runtime; directly transferable.
- **Profiling data for synchronous Rust networking** — cross-platform benchmark artifacts with
  CPU/memory/bandwidth numbers are exactly what's missing for engineers choosing between sync and
  async designs.
- **Public benchmark harnesses** — one-command, commit-pinned, CI-gated; reproducible by design.
- **Documentation for decentralized systems in Rust** — an engineering report and design notes
  that treat decentralized protocols as first-class Rust systems problems.

## Evidence that already exists (all reproducible from a fresh clone)
- 342 tests / 358 run / 16 ignored, zero-warning clippy, 19 benchmarks, 4 fuzz targets —
  pinned in `docs/EVIDENCE_REPORT.md` with direct CI/evidence-run links.
- E1–E9 matrix: churn, partitions, malicious routing, gradient decay, trust ablation,
  neurogenesis (+23–25% bandwidth effect), honest negative result (sparse-gradient).
- Real sockets: 25-process localhost cluster; E2E distributed learning with seed reproducibility.
- 7 pipeline-caught bugs documented as negative results.
- Commit `265e216` (v0.3.1), CI run `30697662079`, evidence run `30697662078`, release tag `v0.3.1`.

## Team & execution
Solo maintainer (age 16 — stated openly) who designed and built the entire system (~21,000 lines,
41 modules) with zero funding on free CI, alongside secondary education; execution plan sized to a
10–15 h/week research schedule. If the Foundation permits team applications with an adult fiscal
sponsor or institutional lead, one is available — the grantee structure must be legitimate and
transparent, never a decorative adult.

## Eligibility note — read before any submission
1. **BLOCKED: applicant under 18.** The official policy requires applicants to be over 18 by the
   date of the grant award. Applicant turns 17 in October 2026. Resolve through: official written
   confirmation of a minor-applicant exception, a legitimate adult fiscal sponsor / institutional
   applicant (not a legal costume), or a future eligible round.
2. **US financial-transfer receipt** must be confirmed for Nepal before submission.
3. **2026 round not yet confirmed open.** Historical Rust Project Grants ranged $2,500–$15,000,
   but no official 2026 application page proving an open round was found as of 2026-08-01.
   Do NOT submit until the exact 2026 round and deadline are verified.
4. Questions to ask the Foundation (grants@rustfoundation.org / contact@rustfoundation.org)
   before any submission: (a) may a team application include an adult fiscal sponsor or
   institutional lead? (b) is the age requirement waivable with guardian consent? (c) can
   financial transfers be received in Nepal?
