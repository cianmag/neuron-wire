# Neuron Wire — Grant Roadmap

**Milestone-based plan for converting a validated simulation and local prototype into a geographically distributed research testbed.**
Companion docs: [GRANT_SUMMARY.md](GRANT_SUMMARY.md) · [GRANT_BUDGET.md](GRANT_BUDGET.md)

Milestone costs are allocations drawn from the $20,000 budget ($17,000 across M1–M7, $3,000 contingency reserved). Durations assume a part-time core team of one maintainer plus community contributors; milestones M1→M5 are sequential, M6 overlaps M4–M5, M7 follows M6.

## Milestones

| # | Milestone | Deliverable | Acceptance criteria | Duration | Cost |
|---|---|---|---|---|---|
| M1 | Green CI and verified alpha | Cross-platform CI green (build + test + clippy + coverage), tagged v0.3.x alpha, README quickstart, known-good validation | 4 CI workflows pass on all supported toolchains; `cargo clippy -- -D warnings` clean; `scripts/reproduce.sh` completes end-to-end; alpha tag with release notes published | 1 week | $1,000 |
| M2 | Reproducible 100-node simulation | Timestamped experiment protocol + committed configs for E1 (100-node convergence, paper-mode); raw CSV results and figures committed | One-command rerun reproduces committed results with the same seed; convergence ≥ 99% under protocol conditions; protocol committed **before** the run | 2 weeks | $500 |
| M3 | Local 25-process test | Real-UDP 25-process localhost harness with scripted churn and failure injection (kill, firewall partition) | 25 concurrent processes converge; failure scenarios recover within bound; per-process metrics captured; OS socket/thread limits documented as findings | 2 weeks | $1,000 |
| M4 | Network emulation benchmark | WAN-condition benchmark (loss 0–10%, jitter, latency 20–300 ms) over ≥ 3 emulated topologies; E6 results incl. negative findings; comparison vs vanilla Kademlia baseline | Published curves for every topology × condition; configs committed; baseline comparison table in repo | 3 weeks | $2,500 |
| M5 | Funded 20-node distributed pilot | 20 nodes across 3 regions (NA/EU/APAC): cloud VPS + low-power edge devices; live public dashboard | 7-day sustained mesh; cross-region gradient delivery loss < 50%; convergence measured under real churn; E7 dataset committed | 6 weeks | $8,000 |
| M6 | Public dataset and research preprint | Open dataset of E1–E7 raw per-tick metrics, reproducibility bundle, preprint with methods + results + negative results; external security review incorporated | Dataset DOI registered; one-command figure reproduction; preprint posted with code links; audit report published in repo | 4 weeks | $2,500 |
| M7 | External reproduction | Third-party reproduction attempt driven via issues; reproduction report merged; protocol stabilized at v1.0 | One independent reproduction run completed — or a blocker report with fixes merged; clean-clone quickstart works; lessons learned committed | 4 weeks | $1,500 |
| — | **Total M1–M7** | | | **22 weeks** | **$17,000** |
| — | Contingency (15% buffer: hardware failure, cloud overruns, extended pilot) | | | | $3,000 |
| — | **Grand total** | | | | **$20,000** |

## 4-week execution plan (pre-funding)

These four weeks produce a credible, evidence-backed package that *is* the grant application.

### Week 1 — Repo credibility
- Make CI green on every supported platform; enforce zero-warning clippy; add README quickstart + badges.
- Triage and fix P0/P1 findings from `docs/AUDIT_REPORT.md`; re-verify `docs/SECURITY_CHECKLIST.md` statuses.
- Tag a clean alpha release; verify mdBook docs build and deploy.
- **Exit criteria:** fresh clone → `cargo build --release && cargo test && cargo clippy -- -D warnings` all clean; tagged release; docs live.

### Week 2 — Research evidence
- Freeze and timestamp the E1–E3 experiment protocol (hypotheses, variables, success criteria) before running anything.
- Run the deterministic simulation suite: 100-node convergence (E1), 100k-node scaling (E2), failure injection incl. malicious peers (E3).
- Commit raw CSVs and generated figures; update STATS.md and the benchmark tables in README.
- **Exit criteria:** reproduction script runs end-to-end; protocol committed before runs; results committed with provenance.

### Week 3 — Funding package
- Finalize GRANT_SUMMARY / GRANT_ROADMAP / GRANT_BUDGET with every number traceable to a committed artifact.
- Draft preprint skeleton (abstract, related work, method); assemble reviewer checklist mapping claims → evidence files.
- **Exit criteria:** package reviewed by 2 external readers; every claim either backed by a committed artifact or explicitly marked pending (⟨E…⟩ placeholder).

### Week 4 — Applications
- Submit the package to 3–5 open-call research grant programs and 1–2 foundations; keep a submission log (program, date, contact, follow-up).
- Register the dataset/DOI plan; open the external-security-review request-for-quote (budget line 3).
- Recruit a volunteer for the M7 reproduction trial; schedule the first independent run.
- **Exit criteria:** submission log with follow-up calendar; security-review RFQ out; reproduction volunteer confirmed.

## Notes
- **No salaries requested** — milestone costs are infrastructure, hardware, review, and publication only; engineering time is contributed in-kind.
- **Evidence discipline** — experiment protocols are committed before execution; negative results are published alongside positive ones; every ⟨E#⟩ placeholder in GRANT_SUMMARY.md resolves to a committed dataset by M6.
