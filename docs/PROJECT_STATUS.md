# Project Status

> **Canonical status entry point.** Read this first. Everything else in the repository is
> supporting detail behind one of the sections below. Last updated: 2026-08-01.

## What this project is
Neuron Wire (NWP) is an MIT-licensed Rust framework for **coordinator-free, peer-to-peer
decentralized learning** — nodes discover each other over a latency-weighted Kademlia DHT,
exchange neural gradients over a custom reliable-UDP transport, and learn collectively with
Hebbian/STDP. Single-threaded non-blocking engine, no async-runtime dependency. It is a
**validated research prototype** (v0.3.1), not a production service.

## Current status (2026-08-01)
| Area | Status |
|------|--------|
| Public CI (Linux/macOS/Windows) | ✅ green |
| Evidence pipeline (342 tests, E1–E9 matrix, 4 fuzz targets) | ✅ green, commit-pinned |
| Evidence provenance | ✅ pinned to `265e216` (v0.3.1), run IDs in §2 of the evidence report |
| Release | ✅ v0.3.1 tag + GitHub release (4 binaries + grant PDF + demo video) |
| Grant package | ✅ complete (PDF + technical summary + submission drafts) |
| Grant submissions | 🔄 in progress — see [tracker](GRANT_SUBMISSIONS/TRACKER.md) |
| WAN deployment | ⏳ funding target (needs the 20-node three-region pilot) |
| External security audit | ⏳ funding target |
| Independent replication | ⏳ funding target — see [reproducibility](../REPRODUCIBILITY.md) |

## Documentation map (strict hierarchy)
1. **`README.md`** — what it is, quickstart, badges. Entry point.
2. **`docs/PROJECT_STATUS.md`** ← you are here. Canonical status.
3. **`docs/ARCHITECTURE.md`** — system architecture and modules.
4. **`docs/WIRE_PROTOCOL.md`** — wire format, reliability tiers, encryption.
5. **`SECURITY.md`** — security posture, supported versions, reporting.
6. **`docs/EVIDENCE_REPORT.md`** — all measured results, commit-pinned, with metric definitions.
7. **`docs/GRANT_SUBMISSIONS/`** — grant drafts + the live submission tracker.
8. **`archive/`** — historical/superseded documents (planned; see below).

**Where the rest lives:** `FORMAL_MODEL.md` and `docs/ALGORITHM_SPEC.md` are the formal
specifications; `ROADMAP.md` is the milestone plan; `GRANT.md` is the grant index;
`docs/TECHNICAL_SUMMARY.md` is the two-page external-facing summary; `docs/GRANT_*` are the
grant-package documents bundled into the release PDF. Files that are superseded (e.g. early
drafts, superseded proposals) should be moved to `archive/` rather than deleted; this migration
is tracked as a cleanup task and does not affect the v0.3.1 release.

## Evidence in one line
342 tests pass on Linux CI, pinned to commit `265e216` (v0.3.1), CI run `30697662079`,
evidence run `30697662078` — full definitions in [EVIDENCE_REPORT.md](EVIDENCE_REPORT.md).

## What's NOT done (honest)
- No WAN deployment, no NAT traversal validation, no external security audit, no independent
  reproduction, no real-task learning benchmark, no bounded-K-bucket routing-table variant,
  no valid network-partition experiment (isolation not verified). Each is a funded-phase task,
  itemized in the evidence report and roadmap — not claimed.
