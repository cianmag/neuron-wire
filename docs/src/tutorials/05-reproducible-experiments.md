# Tutorial 05: Reproducible Experiments

**Goal:** Learn to run deterministic simulations with paper mode, fixed seeds,
multi-trial statistics, and structured CSV output. Recreate the exact
benchmarks from the research paper.

**Estimated time:** 20 minutes

---

## 1. The Problem: Non-Determinism in Distributed Experiments

Distributed systems experiments suffer from three sources of irreproducibility:

1. **Random node IDs** — each run generates fresh NodeIds, changing bucket
   placement and path lengths
2. **Wall-clock timing** — timer jitter, scheduler delays, and clock skew
   change message ordering between runs
3. **Log output** — interleaved print lines from multiple threads corrupt
   structured data

NWP's **paper mode** (`--paper-mode`) eliminates all three:

| Source | Normal Mode | Paper Mode |
|--------|-------------|------------|
| Node IDs | `rand::thread_rng()` | Seeded `StdRng` (default seed=42) |
| Timing | `Instant::now()` timestamps | Relative tick counters |
| Logging | `eprintln!` + JSONL raw logs | Silent (no per-node I/O) |
| Config | CLI defaults | Frozen to `experiment.toml` |
| Output | Raw logs + CSVs | Clean CSVs only |

## 2. Running in Paper Mode

```bash
# Paper-mode single trial
cargo run --release --example simulate -- \
    --paper-mode \
    --nodes 10 \
    --duration 30 \
    --output-dir results/paper-10node
```

### Output Structure

```
results/paper-10node/
├── experiment.toml       # Frozen parameter config (for reproducibility)
├── metadata.json         # Git commit, rustc version, platform, timestamps
├── convergence.csv       # Per-tick peer discovery across nodes
├── bandwidth.csv         # Aggregate bytes/packets in/out per sample
├── routing.csv           # DHT peer count per node over time
├── apoptosis.csv         # Neuron/entry death counts per sweep
├── summary.csv           # One-row aggregation for this trial
└── raw/                  # (empty in paper mode)
    └── .gitkeep
```

### Inspecting the Output

**Summary:**

```bash
cat results/paper-10node/summary.csv
```

```
trial,seed,node_count,duration_secs,total_ticks,total_packets_recv,total_packets_sent,total_bytes_recv,total_bytes_sent,bandwidth_kbps,avg_peers,max_peers,total_apoptosis_deaths,converged,convergence_time_secs
0,42,10,30,30000,482,482,29208,29208,6.8,8.10,9,0,true,3.0
```

**Convergence progression:**

```bash
column -t -s, results/paper-10node/convergence.csv | head -15
```

```
tick  node_0  node_1  node_2  node_3  node_4  node_5  node_6  node_7  node_8  node_9
0     0       0       0       0       0       0       0       0       0       0
100   1       1       1       0       1       1       1       1       1       1
200   1       2       1       1       1       2       2       1       1       2
300   2       3       2       2       3       3       3       3       2       3
400   8       8       8       8       8       8       8       8       8       8
500   9       9       9       9       9       9       9       9       8       9
600   9       9       9       9       9       9       9       9       9       9
```

At tick 600 (~600 ms), all 10 nodes know all 9 peers. The convergence time
(3.0 seconds in the summary) is measured from start to the first tick where
`peers == node_count - 1` for every node.

**Bandwidth:**

```bash
column -t -s, results/paper-10node/bandwidth.csv | head -10
```

```
tick  node_index  bytes_sent  bytes_recv  pkts_sent  pkts_recv
0     0           0           0           0          0
100   0           180         180         3          3
200   0           360         360         6          6
...
```

## 3. Multi-Trial Experiments

Real research requires statistical confidence. Run multiple trials with
different seeds:

```bash
# 10 trials of 5 nodes, 25 seconds each
cargo run --release --example simulate -- \
    --paper-mode \
    --nodes 5 \
    --duration 25 \
    --trials 10 \
    --output-dir results/5node-10trials
```

This spawns 10 sequential trials with seeds 42, 43, 44, ..., 51. The output:

```
results/5node-10trials/
├── experiment.toml
├── metadata.json
├── trial_0/
│   ├── convergence.csv
│   ├── routing.csv
│   └── summary.csv
├── trial_1/
│   └── ...
...
└── trial_9/
    └── ...
```

The simulator prints a summary table with statistics:

```
═══════════════════════════════════════════════════
  NEURON-WIRE DHT CONVERGENCE BENCHMARK
  Nodes: 5 × 25s × 10 trial(s)
═══════════════════════════════════════════════════

  Convergence rate:   10/10 (100%)
  Convergence time:   3.00s ± 0.00s  [min=3.0s, max=3.0s]
  Max peers:          4.00 (of 4)
  Avg peers (steady): 3.52
  Bandwidth:          0.0 kbps avg
  Packets/run:        42 total recv
  ✅ ALL TRIALS CONVERGED
```

Note the **σ = 0.00** for convergence time — because paper mode uses seeded
RNG, every trial converges in exactly the same time for the same seed. With
different seeds, you'd see small variation.

## 4. Seeded vs Paper Mode

Paper mode is a **superset** of seeded mode:

| Flag | Effect |
|------|--------|
| `--seed 123` | Fixes RNG seed to 123, but keeps wall-clock timing and logging |
| `--paper-mode` | Fixes seed=42 (overridable with `--seed`), disables logging, freezes config |

You can combine them:

```bash
# Paper mode with explicit seed (overrides default 42)
cargo run --release --example simulate -- \
    --paper-mode --seed 777 \
    --nodes 5 --duration 15 \
    --output-dir results/seed777
```

## 5. The Frozen Config File

Every paper-mode run exports `experiment.toml` — a complete snapshot of all
parameters:

```toml
[node_count]
value = 10
type = "integer"
description = "Number of simulated nodes"

[duration_secs]
value = 30
type = "integer"
description = "Duration in seconds"

[seed]
value = 42
type = "integer"
description = "RNG seed"

[paper_mode]
value = true
type = "boolean"
description = "Deterministic mode"

[tick_interval_ms]
value = 1
type = "integer"
description = "Engine tick interval"

[gossip_interval_ticks]
value = 10
type = "integer"
description = "Gossip every N ticks"

[gradient_half_life_ms]
value = 100
type = "integer"
description = "Gradient weight decay half-life"
```

This means **anyone can reproduce your exact experiment** by running:

```bash
cargo run --release --example simulate -- --config results/paper-10node/experiment.toml
```

## 6. Metadata

The `metadata.json` file captures the build and platform context:

```json
{
  "git_commit": "095f119",
  "rustc_version": "rustc 1.86.0",
  "platform": "x86_64-unknown-linux-gnu",
  "timestamp": "2026-06-28T15:30:00Z",
  "duration_secs": 30,
  "node_count": 10
}
```

## 7. Validating Against Known-Good Outputs

The repository ships known-good CSV reference files for CI validation:

```bash
# Run validation script
python scripts/validate_repro.py results/paper-10node/ --known-good known-good/
```

Expected output:

```
✅ REPRODUCTION VERIFIED — results match known-good
```

The validator checks these fields:

- `node_count` — must match
- `converged` — must be true
- `conv_time_s` — must match within tolerance
- `max_peers` — must match expected value (`node_count - 1`)
- `avg_peers` — must match within ±5%
- `bw_kbps` — must match within ±10%
- `pkts_recv` — must match within ±10%

## 8. Failure Injection Experiments

The simulator supports failure modes for resilience testing:

```bash
# Node death: kill 30% of nodes at 10 seconds
cargo run --release --example simulate -- \
    --paper-mode --nodes 10 --duration 30 \
    --failure-mode node-death --failure-at 10 --failure-percent 0.3 \
    --output-dir results/node-death

# Network partition: split into two groups at 10 seconds
cargo run --release --example simulate -- \
    --paper-mode --nodes 10 --duration 30 \
    --failure-mode partition --failure-at 10 --failure-percent 0.5 \
    --output-dir results/partition

# Malicious node: one node sends garbage at 8 seconds
cargo run --release --example simulate -- \
    --paper-mode --nodes 10 --duration 30 \
    --failure-mode malicious --failure-at 8 \
    --output-dir results/malicious
```

Each failure mode is documented in `src/simulator.rs`:

| Mode | Effect | Recovery |
|------|--------|----------|
| `node-death` | Kills N% of nodes | Remaining nodes re-converge via DHT |
| `partition` | Splits into two groups | No recovery (intentionally permanent) |
| `malicious` | One node sends garbage | Other nodes detect via fail_count ≥ 3 |

## 9. Expected Results Reference

From `REPRODUCIBILITY.md` — what you should expect:

| Nodes | Duration | Conv Rate | Conv Time | Max Peers | Avg Peers |
|-------|----------|-----------|-----------|-----------|-----------|
| 3     | 10s      | 100%      | 3.0s      | 2/2       | 1.40      |
| 5     | 25s      | 100%      | 3.0s      | 4/4       | 3.52      |
| 10    | 30s      | 100%      | 3.0s      | 9/9       | 8.10      |
| 25    | 30s      | 100%      | 3.0s      | 24/24     | 21.60     |
| 50    | 40s      | 100%      | 4.0s      | 49/49     | 45.35     |

## 10. Try It Yourself

### Quick smoke test (30 seconds)

```bash
cargo run --release --example simulate -- --paper-mode --nodes 3 --duration 10
```

### Compare convergence across scales

```bash
for nodes in 3 5 10 25; do
    cargo run --release --example simulate -- --paper-mode --nodes $nodes --duration 25 \
        --output-dir results/scale-$nodes 2>/dev/null
    echo "--- $nodes nodes ---"
    cat results/scale-$nodes/summary.csv
done
```

You should see convergence time stay constant (~3s) while `avg_peers` scales
nearly linearly with node count.

## Summary

- ✅ **Paper mode** (`--paper-mode`) guarantees deterministic, reproducible
  experiments with seeded RNG, frozen config, and clean CSV output
- ✅ **Multi-trial** with `--trials N` produces per-trial directories and
  aggregate statistics (mean, stddev, min, max)
- ✅ **Output structure** includes `experiment.toml`, `metadata.json`, and
  multiple CSV files for convergence, bandwidth, routing, and apoptosis
- ✅ **Known-good validation** via `scripts/validate_repro.py` ensures
  outputs match reference values
- ✅ **Failure injection** enables resilience experiments (node death,
  partition, malicious)

In **Tutorial 06**, the final tutorial, you'll dive deep into the observability
stack — live metrics, Prometheus scraping, OpenTelemetry distributed tracing,
and the SSE-powered dashboard.

## Source Files Referenced

| File | Lines | Key Content |
|------|-------|-------------|
| [`examples/simulate.rs`](../../examples/simulate.rs) | 1–307 | CLI runner with `--paper-mode`, `--trials`, `--failure-mode` |
| [`src/simulator.rs`](../../src/simulator.rs) | 1–1253 | Core orchestration: `Simulator::launch()`, `run()`, `write_results()` |
| [`REPRODUCIBILITY.md`](../../REPRODUCIBILITY.md) | 1–134 | Full reproducibility guide with expected results table |
| [`ADR 007`](../../adr/007-deterministic-simulation-paper-mode.md) | — | Design rationale for paper mode |
| [`scripts/validate_repro.py`](../../scripts/validate_repro.py) | — | Python validation script |
