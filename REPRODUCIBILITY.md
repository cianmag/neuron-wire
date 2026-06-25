# Reproducibility Guide

**Can another developer run `cargo run --example simulate ...` and get the same results?**
**Yes — and they don't need to know you.**

---

## Quick Start (5 minutes)

```bash
# 1. Clone
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire

# 2. Build & run paper-mode benchmark
cargo run --example simulate -- --paper-mode --nodes 3 --duration 10

# 3. Verify against known-good
pip install -r scripts/requirements.txt  # optional (just needs python3)
python scripts/validate_repro.py results/experiment_*/ --known-good known-good/
```

Expected output:
```
✅ REPRODUCTION VERIFIED — results match known-good
```

---

## What "Reproducible" Means

The DHT convergence benchmark is **deterministic** given the same seed.

| Property | Guarantee |
|----------|-----------|
| **Seed-driven RNG** | Same `--seed` → identical node IDs, identical timing |
| **Paper mode** (`--paper-mode`) | Forces `seed=42`, disables non-deterministic logging |
| **Synchronized start** | All nodes launched before simulation begins |
| **No external dependencies** | Pure Rust + stdlib + UDP loopback (no Tokio, no DB) |
| **Cross-platform** | Tested on Windows 10, should work on Linux/macOS |

---

## Full Benchmark Suite

Reproduce every result in the paper:

```bash
# Fast smoke test (30s)
cargo run --example simulate -- --paper-mode --nodes 3 --duration 10

# 5-node benchmark (25s)
cargo run --example simulate -- --paper-mode --nodes 5 --duration 25 --output-dir results/5node-repro

# 10-node benchmark (30s)
cargo run --example simulate -- --paper-mode --nodes 10 --duration 30 --output-dir results/10node-repro

# 25-node benchmark (30s)
cargo run --example simulate -- --paper-mode --nodes 25 --duration 30 --output-dir results/25node-repro

# 50-node benchmark (40s — requires ~8 CPU cores)
cargo run --example simulate -- --paper-mode --nodes 50 --duration 40 --output-dir results/50node-repro
```

### Multi-trial statistics

```bash
# 10 trials of 5 nodes (4 min)
cargo run --example simulate -- --paper-mode --nodes 5 --duration 25 --trials 10
```

---

## Expected Results

| Nodes | Duration | Conv Rate | Conv Time | Max Peers | Avg Peers |
|-------|----------|-----------|-----------|-----------|-----------|
| 3     | 10s      | 100%      | 3.0s      | 2/2       | 1.40      |
| 5     | 25s      | 100%      | 3.0s      | 4/4       | 3.52      |
| 10    | 30s      | 100%      | 3.0s      | 9/9       | 8.10      |
| 25    | 30s      | 100%      | 3.0s      | 24/24     | 21.60     |
| 50    | 40s      | 100%      | 4.0s      | 49/49     | 45.35     |

**Convergence is defined as** the first sample where every node knows every other node
in its routing table. Reported values are mean across trials.

---

## Validation

The validation script compares simulation output against known-good CSVs:

```bash
python scripts/validate_repro.py <output-dir> --known-good known-good/
```

Exits with code 0 on match, 1 on mismatch with a diff report.
Checked fields: `{nodes, converged, conv_time_s, max_peers, avg_peers, bw_kbps, pkts_recv}`.

### Reference files

Known-good values are stored in `known-good/`:

- `known-good/benchmark-3node-paper.csv`
- `known-good/benchmark-5node-paper.csv`
- `known-good/benchmark-5node-paper-convergence.csv` (full per-tick peer progression)
- `known-good/benchmark-10node-paper.csv`

These are the same files the CI runner checks against.

---

## CI (GitHub Actions)

Every push runs:

1. `cargo build` — must compile without errors
2. `cargo test` — all 60+ unit tests
3. `cargo run --example simulate -- --paper-mode --nodes 3 --duration 10` — quick benchmark
4. `python scripts/validate_repro.py` — verifies against known-good

Badge: [![build](https://github.com/cianmag/neuron-wire/actions/workflows/ci.yml/badge.svg)](https://github.com/cianmag/neuron-wire/actions)

---

## Live Dashboard

The benchmark results power a live public dashboard:

**https://neuron-wire-dashboard.vercel.app**

Shows: animated network topology, convergence time scaling, bandwidth scaling,
peer discovery timeline, full results table, churn statistics.
All data sourced from the same benchmark CSVs in `results/`.
