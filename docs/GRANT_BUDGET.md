# Neuron Wire — Grant Budget

**$20,000 request: converting a validated simulation and local prototype into a geographically distributed research testbed.**
Companion docs: [GRANT_SUMMARY.md](GRANT_SUMMARY.md) · [GRANT_ROADMAP.md](GRANT_ROADMAP.md)

## Budget table

| # | Line item | Amount |
|---|---|---|
| 1 | Cloud test infrastructure | $4,000 |
| 2 | Low-power devices and networking hardware | $3,000 |
| 3 | Security review | $5,000 |
| 4 | Research compute and storage | $3,000 |
| 5 | Domain / monitoring / project services | $1,000 |
| 6 | Documentation and publication | $1,000 |
| 7 | Contingency | $3,000 |
| | **Total** | **$20,000** |

## Justifications

**1. Cloud test infrastructure — $4,000.** The core deliverable is a 20-node, 3-region pilot (M5): ~$1,000/month for three months of small VPS instances in North America, Europe, and Asia-Pacific, plus two larger hosts for network-emulated WAN conditions (M4). This is the difference between "converges on localhost" and "converges on the actual Internet" — the central question the grant answers.

**2. Low-power devices and networking hardware — $3,000.** A pilot confined to datacenter VPSs would not test the project's core claim: *any device that can reach another device can participate.* A set of Raspberry Pi-class edge nodes, a managed switch, and networking accessories place real NWP nodes on constrained hardware (the 512 MB target), exercise the single-threaded engine's idle efficiency, and directly support the M3 25-process local test and M5 edge-node deployment.

**3. Security review — $5,000.** The protocol authenticates every packet (Ed25519), encrypts payloads (XChaCha20-Poly1305 AEAD), and maintains a hash-chained audit log — but these primitives were designed and reviewed in-house. `docs/SECURITY_CHECKLIST.md` lists open items (key zeroization, traffic-analysis resistance, per-peer memory caps) that deserve an independent cryptographic review before the protocol claims production-readiness. An external audit report is a prerequisite for credible external reproduction (M7) and for the preprint (M6). This is the largest single line item because independent security review is the highest-value external validation available at this stage.

**4. Research compute and storage — $3,000.** Long-running deterministic simulation sweeps (failure injection, churn, scaling — E2–E4), the network-emulation benchmark (E6), and the distributed-learning baseline comparisons (E9) need dedicated batch compute; raw per-tick experiment data needs durable, versioned storage so the public dataset (M6) ships with provenance, not just aggregates.

**5. Domain / monitoring / project services — $1,000.** Domain and TLS for the public pilot dashboard, hosted Prometheus/Grafana for live per-region metrics (28+ metrics already exported per node), CI minutes beyond free-tier limits, and project-management tooling for the milestone tracker and submission log.

**6. Documentation and publication — $1,000.** Open-access publication fee for a workshop track (or preprint hosting + DOI if fees are waived), DOI registration for the E1–E7 dataset, mdBook hosting, and figure/paper production for the preprint (M6).

**7. Contingency — $3,000 (15%).** Buffer for hardware failure, cloud cost overruns, an extended pilot window, or a second security-review pass if the first finds material issues. Unspent contingency returns to the funder.

**Not in this budget:** salaries (engineering time is contributed in-kind by the maintainer and community), conference travel, legal fees, and anything not directly serving the M1–M7 milestones.

## Smaller alternative packages

### $10,000 version — "single-region evidence"
| Line item | Amount |
|---|---|
| Cloud test infrastructure (1 region, emulation only) | $2,000 |
| Low-power devices and networking hardware | $1,500 |
| Security review (targeted cryptographic review, not full audit) | $2,500 |
| Research compute and storage | $1,500 |
| Domain / monitoring / project services | $500 |
| Documentation and publication | $500 |
| Contingency | $1,500 |
| **Total** | **$10,000** |

**What changes:** pilot shrinks to ~12 nodes in a single region with emulated inter-region conditions (E6 covers the gap); the security review becomes a scoped cryptographic review of the transport and trust subsystems rather than a full protocol audit; dataset and preprint still ship (M6) but without the audit chapter.

### $5,000 version — "local evidence, preprint only"
| Line item | Amount |
|---|---|
| Cloud test infrastructure (emulation hosts only) | $1,000 |
| Low-power devices and networking hardware | $750 |
| Security review (in-house hardening + open-source review call) | $1,250 |
| Research compute and storage | $750 |
| Domain / monitoring / project services | $250 |
| Documentation and publication | $250 |
| Contingency | $750 |
| **Total** | **$5,000** |

**What changes:** M5 becomes a local 25-process + emulated-WAN testbed (M3–M4) with no real distributed pilot; no independent security audit — instead a documented in-house hardening pass and a public review request; publication is preprint-only with no paid open-access venue. This package strengthens the *evidence* but cannot produce the geographically distributed deployment results that are the project's open question.
