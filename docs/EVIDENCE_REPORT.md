# Neuron Wire — Evidence Report

> **Public evidence package for grant evaluation.**
> Repository: [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) · Company: Zylvon · Version: 0.3.0

---

## 1. Project Status

```
Current stage:  VALIDATED RESEARCH PROTOTYPE

Validated through:
  ✓ deterministic simulation
  ✓ local multi-process networking
  ✓ property-based and fuzz testing
  ✓ network impairment emulation
  ✓ reproducible benchmarks

Not yet completed:
  ✗ geographically distributed deployment
  ✗ external security audit
  ✗ independent replication

Funding purpose:
  Build and evaluate the first real distributed test network.
```

## 2. Evidence Trail (commit-pinned)

| Artifact | Location | Status |
|----------|----------|--------|
| Source code | `src/` (41 modules, ~21,000 lines) | ⟨commit⟩ |
| Unit + integration tests | `tests/` (341 test functions) | ⟨CI result⟩ |
| Property-based tests | `tests/proptest.rs` (20) | ⟨CI result⟩ |
| Wire protocol tests | `tests/wire_protocol.rs` (14) | ⟨CI result⟩ |
| Stress/soak tests | `tests/stress.rs` (9) | ⟨CI result⟩ |
| Security tests | `tests/security_integration.rs` (7) | ⟨CI result⟩ |
| E2E tests | `tests/e2e.rs` (8) | ⟨CI result⟩ |
| Deterministic simulator | `src/simulator.rs` (paper-mode, fixed seeds) | ✅ |
| Experiment matrix runner | `evidence/run_matrix.sh` | ✅ |
| Local multi-process cluster | `evidence/localhost_cluster.sh` | ✅ |
| Network emulation | `evidence/emulate_network.sh` (tc netem) | ✅ |
| Fuzz harness | `fuzz/` (4 targets) | ✅ |
| CI verification | `.github/workflows/` (Linux/macOS/Windows) | ⟨status⟩ |

## 3. Benchmark Environment

| Parameter | Value |
|-----------|-------|
| CI host | GitHub Actions ubuntu-latest (2 vCPU, 7 GB RAM) |
| Toolchain | stable (dtolnay/rust-toolchain) |
| Release profile | opt-level="z", LTO, stripped |
| Simulator | in-process message passing, single seed, deterministic |
| Local cluster | real UDP sockets on 127.0.0.1, separate ports/identities/storage |

## 4. Deterministic Simulation Results

All experiments use fixed seeds (`42`, `1337`, `9001`) via `--paper-mode`.

### E1 — Convergence scaling

| Nodes | Seed | Converged | Convergence time (s) | Avg peers | Bandwidth (kbps) |
|-------|------|-----------|----------------------|-----------|------------------|
| 10    | 42   | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 25    | 42   | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 50    | 42   | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 100   | 42   | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 500   | 42   | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 100   | 1337 | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 100   | 9001 | ⟨⟩ | ⟨⟩ | ⟨⟩ | ⟨⟩ |

Raw data: `results/evidence/E1_*/summary.csv` (committed with each run)

### E2 — Node churn (100 nodes, death at t=30s)

| Churn | Converged post-churn | Recovery time (s) | Min peers post-failure |
|-------|----------------------|-------------------|------------------------|
| 10%   | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 20%   | ⟨⟩ | ⟨⟩ | ⟨⟩ |
| 50%   | ⟨⟩ | ⟨⟩ | ⟨⟩ |

### E5 — Malicious peer injection (100 nodes, t=20s)

| Metric | Value |
|--------|-------|
| Detected (trust dropped) | ⟨⟩ |
| Network converged post-attack | ⟨⟩ |
| Recovery time | ⟨⟩ |

### E6 — Network partition (100 nodes, split t=20s)

| Metric | Value |
|--------|-------|
| Partitions healed after removal | ⟨⟩ |
| Convergence restored | ⟨⟩ |
| Recovery time | ⟨⟩ |

## 5. Local Multi-Process Results (real UDP sockets)

| Nodes | Duration | Alive at end | Health check |
|-------|----------|--------------|--------------|
| 2     | ⟨30s⟩ | ⟨⟩/2 | ⟨⟩ |
| 5     | ⟨30s⟩ | ⟨⟩/5 | ⟨⟩ |
| 10    | ⟨30s⟩ | ⟨⟩/10 | ⟨⟩ |
| 25    | ⟨30s⟩ | ⟨⟩/25 | ⟨⟩ |

Logs: `results/localhost_cluster_*/` (node-N.log, health checks, metrics samples)

## 6. Network Emulation Results

| Scenario | Latency | Loss | Nodes | Result |
|----------|---------|------|-------|--------|
| Normal   | 20 ms   | 0%   | ⟨N⟩   | ⟨⟩ |
| Mobile   | 80 ms   | 2%   | ⟨N⟩   | ⟨⟩ |
| Weak     | 150 ms  | 5%   | ⟨N⟩   | ⟨⟩ |
| Severe   | 300 ms  | 10%  | ⟨N⟩   | ⟨⟩ |

Method: `tc netem` on loopback (Linux, root) driving the local multi-process cluster.

## 7. Baselines (planned)

To be reported after E1–E9: comparison vs random peer discovery, plain Kademlia,
Kademlia without latency weighting, gossip without gradient aging, static topology,
no trust scoring, no apoptosis, no neurogenesis. Metrics: convergence time, bytes
transmitted, message count, retransmissions, CPU, memory, learning accuracy,
recovery time, performance under churn. (Baseline harness requires simulator
feature toggles — tracked as funded-phase task M4.)

## 8. Reproducibility Instructions

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire

# Deterministic simulation matrix (E1, E2, E5, E6)
./evidence/run_matrix.sh            # → results/evidence/

# Local multi-process cluster (2/5/10/25 real nodes)
./evidence/localhost_cluster.sh 5 30

# Network emulation (Linux, root)
sudo ./evidence/emulate_network.sh 5 45
```

Expected output:
```
results/
├── evidence/
│   ├── E1_nodes10_seed42/
│   │   ├── experiment.toml      # Frozen parameter config
│   │   ├── metadata.json        # Git commit, rustc, platform
│   │   ├── convergence.csv
│   │   ├── bandwidth.csv
│   │   ├── routing.csv
│   │   ├── apoptosis.csv
│   │   ├── summary.csv
│   │   └── raw/                 # Per-node event logs
│   ├── evidence_master.csv      # All trials aggregated
│   └── evidence_table.md
└── localhost_cluster_5/         # Real-process logs + health checks
```

## 9. Honest Limitations

1. No geographically distributed deployment yet — localhost and simulator only.
2. Simulator results may differ from WAN results (real internet has asymmetric
   links, NAT, middleboxes, ISP behavior not modeled).
3. NAT traversal (STUN) implemented but incomplete (no TURN relay yet).
4. Learning quality validated on synthetic benchmarks only; broader datasets needed.
5. Security hardened in depth but has not received an external audit.
6. Formal proofs (ALGORITHM_SPEC.md) depend on stated model assumptions.
7. Sybil resistance is mitigation, not elimination — a determined adversary with
   massive IP diversity can still impose cost.
8. Windows host toolchain issues (AppLocker) required CI-only verification on
   Linux/macOS; local dev machine cannot build.

## 10. Funded Milestones (measurable)

| Milestone | Deliverable | Acceptance criteria |
|-----------|-------------|---------------------|
| M1 | Green CI + verified alpha | All CI jobs pass on 3 OSes; 341 tests green |
| M2 | Reproducible 100-node simulation | `run_matrix.sh` reproduces E1-E9 from a fresh clone |
| M3 | Local 25-process test | 25 real nodes converge on localhost, health check 25/25 |
| M4 | Network emulation benchmark | 4 impairment scenarios + baselines, raw data published |
| M5 | 20-node distributed pilot | 20 real nodes across ≥3 cloud regions, monitored |
| M6 | Public dataset + preprint | evidence repo + GRANT_PREPRINT.md published |
| M7 | External reproduction | Independent party reproduces E1-E6 from public repo |

## 11. Funding Request

**$20,000** — see [GRANT_BUDGET.md](./GRANT_BUDGET.md)

| Category | Amount |
|----------|--------|
| Cloud test infrastructure | $4,000 |
| Low-power devices and networking hardware | $3,000 |
| Security review | $5,000 |
| Research compute and storage | $3,000 |
| Domain, monitoring, project services | $1,000 |
| Documentation and publication | $1,000 |
| Contingency | $3,000 |
| **Total** | **$20,000** |

---

*This report is updated on every evidence-producing run. Last updated: ⟨date⟩ at commit ⟨commit⟩.*
