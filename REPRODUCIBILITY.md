# Reproducibility Guide

**Can another developer clone this repo and get the same results?**
**Yes — one command, guaranteed.**

```bash
bash scripts/reproduce.sh
```

That single command:
1. Captures full environment (compiler, OS, CPU, deps, source hashes)
2. Builds the release binary
3. Runs every experiment in `experiments/`
4. Validates output against `known-good/` reference data
5. Generates publication-ready figures

---

## One-Command Reproduction

```bash
# Full reproduction (all figures)
bash scripts/reproduce.sh

# Individual figure
bash scripts/reproduce.sh --figures 1

# Range of figures
bash scripts/reproduce.sh --figures 1-5

# Validate against known-good only (fast)
bash scripts/reproduce.sh --validate

# Regenerate plots from existing data (no re-run)
bash scripts/reproduce.sh --plots-only
```

### Output Structure

```
results/
├── env/                          # Full environment capture
│   └── 20260701-120000/
│       ├── metadata.json         # Consolidated JSON
│       ├── commit.txt            # Git commit hash (SHA)
│       ├── branch.txt
│       ├── rustc.txt             # rustc --version
│       ├── cargo.txt             # cargo --version
│       ├── rustup.txt            # Active toolchain
│       ├── os.txt                # OS name/version
│       ├── kernel.txt            # uname -a
│       ├── cpu.txt               # CPU model name
│       ├── cpu_cores.txt         # Core count
│       ├── memory.txt            # Total RAM
│       ├── hostname.txt
│       ├── dependencies.txt      # Full cargo tree
│       ├── dependencies-compact.txt
│       └── source-hashes.txt     # SHA256 of every .rs file
├── figure-1-dht-convergence-3node/
│   ├── experiment.toml           # Frozen config snapshot
│   ├── metadata.json             # Per-run metadata
│   ├── summary.csv               # Per-trial aggregation
│   ├── convergence.csv           # Per-tick peer convergence
│   ├── bandwidth.csv             # Bandwidth over time
│   ├── routing.csv               # DHT peer counts
│   ├── apoptosis.csv             # Death counts
│   ├── output-hashes.txt         # SHA256 of all outputs
│   └── raw/                      # Per-node JSONL logs
├── figure-2-.../
│   └── ...                       # Same structure
├── figures/                      # Publication-ready plots (PNG/PDF/SVG)
│   ├── figure-1-*.png
│   ├── figure-2-*.png
│   └── ...
└── reproduction-summary-*.csv    # One-row-per-figure status table
```

---

## What "Reproducible" Means

Every experiment output directory captures **everything** needed to prove nothing was fabricated:

| Artifact | What It Contains |
|----------|-----------------|
| `experiment.toml` | Every configuration parameter frozen at experiment time |
| `metadata.json` | Git commit, rustc version, OS, CPU, RAM, hostname, timestamp |
| `summary.csv` | Per-trial aggregation (converged, conv_time, bandwidth, peers, packets) |
| `convergence.csv` | Per-tick peer progression for every node |
| `bandwidth.csv` | Aggregate bytes in/out per second |
| `routing.csv` | DHT peer count per node over time |
| `apoptosis.csv` | Death counts per apoptosis sweep |
| `raw/node_NNN.jsonl` | Per-node event log (if `include_raw_logs=true`) |
| `output-hashes.txt` | SHA256 of every output file — verifiable independence |
| `source-hashes.txt` | SHA256 of every `.rs` file in `src/` |

**If someone questions the results, they get:**
- The exact commit (`git checkout <hash>`)
- The exact compiler (`rustc <version>`)
- The exact config (`experiment.toml`)
- The exact OS and hardware (`os.txt`, `cpu.txt`, `memory.txt`)
- The exact output hashes to verify integrity
- A one-command script to regenerate everything from scratch

---

## Validation

The validation script compares simulation output against known-good CSVs:

```bash
python3 scripts/validate_repro.py <results-dir> --known-good known-good/
```

Exits with code 0 on match, 1 on mismatch with a diff report.
Checked fields: `{nodes, converged, conv_time_s, max_peers, avg_peers, bw_kbps, pkts_recv}`.

### Reference files

Known-good values are stored in `known-good/`:

```
known-good/
├── figure-1-dht-convergence-3node/
│   ├── summary.csv
│   └── convergence.csv
├── figure-2-dht-convergence-10node/
│   └── ...
└── ...
```

These are the same files the CI runner checks against. To update known-good after a verified change:

```bash
bash scripts/reproduce.sh
cp -r results/figure-* known-good/
git add known-good/
```

---

## Experiments

Pre-configured experiment files live in `experiments/`:

| File | Description | Runtime |
|------|-------------|---------|
| `figure-1-dht-convergence-3node.toml` | 3-node DHT convergence baseline | ~10s |
| `figure-2-dht-convergence-10node.toml` | 10-node DHT convergence scaling | ~30s |
| `figure-3-dht-convergence-25node.toml` | 25-node DHT convergence scaling | ~30s |
| `figure-4-convergence-time-scaling.toml` | Convergence time vs node count | ~60s |
| `figure-5-bandwidth-vs-nodes.toml` | Bandwidth scaling with network size | ~30s |
| `figure-6-node-churn-recovery.toml` | Node churn and recovery behavior | ~60s |
| `figure-7-failure-injection.toml` | Adversarial failure injection | ~45s |
| `figure-8-multi-trial-stats.toml` | Multi-trial statistical analysis | ~5min |
| `figure-9-dashboard-visualization.toml` | Dashboard visualization benchmark | ~25s |
| `figure-10-sga-comparison.toml` | SGA vs Fixed gossip comparison | ~30s |

---

## CI

Every push runs the reproducibility pipeline:

```yaml
# .github/workflows/repro.yml
- cargo build --release
- bash scripts/reproduce.sh --figures 1   # Fast smoke test
- python3 scripts/validate_repro.py results/ --known-good known-good/
```

Badges:
[![repro](https://github.com/cianmag/neuron-wire/actions/workflows/repro.yml/badge.svg)](https://github.com/cianmag/neuron-wire/actions)

---

## Environment Captured

| Field | Source | Format |
|-------|--------|--------|
| Git commit | `git rev-parse HEAD` | SHA (40 hex chars) |
| Git branch | `git rev-parse --abbrev-ref HEAD` | String |
| Git tag | `git describe --tags --exact-match` | String |
| Uncommitted changes | `git status --porcelain \| wc -l` | Integer |
| Repository URL | `git remote get-url origin` | URL |
| Rust compiler version | `rustc --version` | String |
| Cargo version | `cargo --version` | String |
| Active toolchain | `rustup show active-toolchain` | String |
| OS name/version | `/etc/os-release` or `sw_vers` | String |
| Kernel | `uname -a` | String |
| CPU model | `/proc/cpuinfo` or `sysctl` | String |
| CPU cores | `/proc/cpuinfo \| grep processor \| wc -l` | Integer |
| Total RAM | `/proc/meminfo` or `sysctl` | String |
| Hostname | `hostname` | String |
| Full dependency tree | `cargo tree --prefix depth` | Text |
| Source file hashes | `sha256sum src/**/*.rs` | SHA256 |
| Output file hashes | `sha256sum results/**/*.csv` | SHA256 |

---

## Scripts

| Script | Purpose |
|--------|---------|
| `scripts/reproduce.sh` | One-command full reproduction |
| `scripts/capture_env.sh` | Full environment metadata capture |
| `scripts/generate_figures.py` | Publication-ready figure generation |
| `scripts/validate_repro.py` | Output comparison against known-good |
| `scripts/check_bench_regressions.py` | Benchmark regression detection |

---

## Quick Reference

```bash
# 1. Clone
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire

# 2. Full reproduction (30-60 min, all figures)
bash scripts/reproduce.sh

# 3. Smoke test (10s)
bash scripts/reproduce.sh --figures 1

# 4. Check results
ls results/
ls results/figures/

# 5. Verify integrity
cat results/env/*/source-hashes.txt
cat results/figure-1-*/output-hashes.txt
```
