# Filecoin Foundation — One-Page Concept Note (go/no-go)

**Status:** CONCEPT ONLY — not a full application · verified 2026-08-01
**Program:** Filecoin Foundation dev grants — "Builder Next Step" (≈$5k–$10k)
**Rule applied (per review):** do NOT bolt Filecoin onto the proposal. This concept is only worth
pursuing if (a) the program confirms the scope qualifies, AND (b) the integration is worth
building independently of the grant. Otherwise: no submission.

**Applicant:** Zylvon · **Project:** Neuron Wire (NWP), v0.3.1 evidence release

---

## Title
**Verifiable Storage and Reproduction for Coordinator-Free AI Experiments**

## What it is (one page, honest scope)
Neuron Wire is a Rust P2P infrastructure layer for decentralized learning, currently validated as a
reproducible research prototype (342 tests, E1–E9 matrix, every result pinned to commit
`265e216`). Today, experiment artifacts live on GitHub + CI. This concept makes the evidence
trail **content-addressed and persistent** using the Filecoin/IPFS ecosystem:

1. **Experiment manifests** — every evidence run writes an IPLD manifest (commit, CI/evidence run
   IDs, seeds, config, checksums) to IPFS; the manifest CID is recorded in the repo.
2. **Signed results + immutable evidence packages** — hash-chained result records (mirroring NWP's
   existing audit-log design) stored as immutable datasets, so "this result came from this commit"
   is cryptographically verifiable long after CI artifacts expire.
3. **Model checkpoints / reproducibility datasets** — gradient checkpoints and benchmark artifacts
   stored under CID; one-command reproduction pulls the exact dataset instead of rebuilding.
4. **Long-horizon persistence** — Filecoin deals for the reproducibility datasets, so the evidence
   trail survives repo churn and free-tier CI retention limits.

## Why this is a real integration, not decoration
- NWP already has a hash-chained tamper-evident audit log and a provenance discipline (one commit
  per evidence release). Content-addressed storage is the natural continuation — the protocol
  treats identity as `SHA-256(pubkey)` and the evidence system already links CIDs to results.
- The deliverable benefits the Filecoin ecosystem: a documented pattern for "verifiable
  reproduction of distributed-learning experiments" using IPLD + Filecoin deals, usable by any
  research project, not just NWP.
- Deliverables: `nwp-archive` crate/module (manifest → CID → deal workflow), a public archive of
  the v0.3.1 evidence package, and a written pattern doc.

## Go/no-go criteria (evaluate before any submission)
| # | Criterion | Needed to proceed |
|---|-----------|-------------------|
| 1 | Program confirms this scope qualifies for Builder Next Step | Written/email confirmation |
| 2 | Integration worth building without the grant | Yes — NWP would adopt content-addressed evidence regardless |
| 3 | Payout terms acceptable | Asset, exchange-rate date, vesting, custody, tax, minor-receipt, fiat conversion all defined in the award agreement |
| 4 | No ecosystem cosplay | The proposal stands on NWP's own merits; Filecoin is the storage substrate, not the rationale |

## Not requested
No budget is requested in this concept note. If go criteria pass, a scoped milestone budget
(≤$10k) will be drafted with the storage deal costs itemized.

## Contact
[Founder name + email] · github.com/cianmag/neuron-wire · evidence: `docs/EVIDENCE_REPORT.md`
