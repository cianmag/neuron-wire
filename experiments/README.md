# Experiment Configurations

This directory contains pre-configured experiment TOML files for every figure in the neuron-wire paper series.

## Usage

```bash
# Reproduce a single figure
cargo run --example simulate -- --config experiments/figure-1-dht-convergence-3node.toml

# Reproduce all figures (via reproduce.sh)
bash scripts/reproduce.sh
```

## Figures

| File | Figure | Description | Runtime |
|------|--------|-------------|---------|
| `figure-1-dht-convergence-3node.toml` | Figure 1 | 3-node DHT convergence baseline | ~10s |
| `figure-2-dht-convergence-10node.toml` | Figure 2 | 10-node DHT convergence scaling | ~30s |
| `figure-3-dht-convergence-25node.toml` | Figure 3 | 25-node DHT convergence scaling | ~30s |
| `figure-4-convergence-time-scaling.toml` | Figure 4 | Convergence time vs node count | ~60s |
| `figure-5-bandwidth-vs-nodes.toml` | Figure 5 | Bandwidth scaling with network size | ~30s |
| `figure-6-node-churn-recovery.toml` | Figure 6 | Node churn and recovery behavior | ~60s |
| `figure-7-failure-injection.toml` | Figure 7 | Adversarial failure injection | ~45s |
| `figure-8-multi-trial-stats.toml` | Figure 8 | Multi-trial statistical analysis | ~5min |
| `figure-9-dashboard-visualization.toml` | Figure 9 | Dashboard visualization benchmark | ~25s |
| `figure-10-sga-comparison.toml` | Figure 10 | SGA vs Fixed gossip comparison | ~30s |

## Adding a New Figure

1. Create `<figure-name>.toml` with the experiment parameters
2. Run it once to generate baseline outputs
3. Copy outputs to `known-good/<figure-name>/` for CI validation
4. Add to `scripts/reproduce.sh`
5. Add to this README

## Config Reference

| Section | Key | Description | Default |
|---------|-----|-------------|---------|
| `experiment` | `name` | Unique experiment identifier | — |
| `experiment` | `paper_mode` | Enable deterministic mode | `true` |
| `experiment` | `seed` | RNG seed | `42` |
| `experiment` | `trials` | Number of trials | `1` |
| `simulation` | `node_count` | Nodes in the simulation | — |
| `simulation` | `duration_secs` | Duration in seconds | — |
| `simulation` | `stale_ping_secs` | DHT stale ping threshold | `10` |
| `simulation` | `tick_interval_us` | Tick interval in microseconds | `1000` |
| `failure` | `mode` | Failure injection mode | — |
| `failure` | `at_secs` | When to inject failure | — |
| `failure` | `percent` | Failure extent (0.0-1.0) | — |
| `sga` | `enabled` | Enable Sparse Gossip Aging | `false` |
| `sga` | `aging_factor` | Gossip aging multiplier | `0.7` |
| `output` | `directory` | Output directory | `results/` |
| `output` | `include_raw_logs` | Include per-node JSONL logs | `false` |
| `output` | `snapshot_interval_ticks` | Metrics snapshot interval | `100` |
