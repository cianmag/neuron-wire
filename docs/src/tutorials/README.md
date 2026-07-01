# Neuron Wire Tutorials

**Learn the Neuron Wire Protocol (NWP) — a decentralised adaptive runtime for
large-scale distributed learning.**

---

This tutorial series walks you through the Neuron Wire Protocol from first
principles to advanced observability. Each tutorial builds on the previous one,
containing practical commands, code snippets, and clear explanations grounded
in the actual source code.

## Prerequisites

- **Rust toolchain** (edition 2021, tested with rustc 1.80+)
- **Git** for cloning the repository
- **A terminal** — all commands are POSIX-shell compatible
- **Basic familiarity** with Rust, UDP networking, and P2P concepts (helpful
  but not required)

## Tutorials

| # | Tutorial | Description | Est. Time |
|---|----------|-------------|-----------|
| 1 | [Getting Started](01-getting-started.md) | Clone, build, run your first node, explore the live dashboard | 15 min |
| 2 | [Multi-Node Network](02-multi-node-network.md) | Launch three nodes on localhost, watch DHT discovery, observe packet exchange | 20 min |
| 3 | [Engine Loop & Tick Model](03-engine-loop-tick-model.md) | Understand the six-phase tick, deterministic vs real-time modes, backpressure | 20 min |
| 4 | [DHT Routing Deep Dive](04-dht-routing.md) | Kademlia with latency-weighted buckets, bootstrap sequence, routing table visualisation | 25 min |
| 5 | [Reproducible Experiments](05-reproducible-experiments.md) | Paper mode, fixed seeds, CSV output, multi-trial with the simulator harness | 20 min |
| 6 | [Observability Deep Dive](06-observability-deep-dive.md) | Metrics, Prometheus endpoint, OpenTelemetry traces, dashboard SSE stream | 25 min |

## Quick Reference

```bash
# Clone & build
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release

# Run all tests
cargo test

# Launch a 3-node simulation (10 seconds)
cargo run --example simulate -- --nodes 3 --duration 10

# Paper-mode reproduction (deterministic, seed=42)
cargo run --example simulate -- --paper-mode --nodes 5 --duration 25 --output-dir results/my-experiment

# Explore CSV output
column -t -s, results/my-experiment/routing.csv | head -20
```

## How to Use These Tutorials

1. **Start with Tutorial 01** — it gets you a running node and a dashboard.
2. **Run every command** — each tutorial contains copy-pasteable commands that
   work on a standard laptop.
3. **Inspect the output** — CSV files, terminal logs, and dashboard screenshots
   are part of the learning path.
4. **Cross-reference** — each tutorial links to the relevant source files
   (`src/engine_loop.rs`, `src/dht.rs`, etc.) and architectural documents
   (`ARCHITECTURE.md`, `REPRODUCIBILITY.md`, etc.).

## Key Concepts

| Concept | Description | Where to Learn |
|---------|-------------|----------------|
| **Engine Loop** | Single-threaded, six-phase tick loop (~1 MHz on commodity hardware) | Tutorial 03 |
| **DHT** | Modified Kademlia with XOR routing + latency-weighted eviction (K=20, 256 buckets) | Tutorial 04 |
| **UDP Transport** | Custom reliable transport with 3 tiers (BestEffort, Data, Consensus) + gradient decay | Tutorials 02 & 03 |
| **Neural Computation** | Forward pass (tanh propagation) + Hebbian STDP (η=0.01, λ=0.999) | Tutorials 03 & 04 |
| **Paper Mode** | Deterministic simulation with seeded RNG, frozen config, CSV export | Tutorial 05 |
| **Observability** | Metrics (counters + gauges), Prometheus `/metrics`, OTel spans, SSE dashboard | Tutorial 06 |

## 💡 Tips

- **Watch logs closely** — the engine prints `[ENGINE] tick=... rate=... peers=...`
  lines that tell you the node is alive and connected.
- **Use `--paper-mode` for clean CSV output** — without it, nodes emit verbose
  JSONL event logs that are great for debugging but harder to parse.
- **The dashboard** (`http://localhost:9090`) updates every second via SSE and
  shows live topology, bandwidth, and convergence charts.
- **If a command fails**, check that you're in the `neuron-wire/` directory and
  that `cargo build --release` completed without errors.

---

*Happy hacking — and remember, every neuron in this network learns on its own.*
