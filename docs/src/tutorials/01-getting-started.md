# Tutorial 01: Getting Started

**Goal:** Clone the neuron-wire repository, build the binary, run a single node,
and explore the live dashboard.

**Estimated time:** 15 minutes

---

## 1. Clone and Explore

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
```

Take a look at what you've got:

```bash
ls -F
```

You'll see the source tree (under `src/`), documentation (`README.md`,
`ARCHITECTURE.md`, `REPRODUCIBILITY.md`, etc.), an `examples/` directory with
the simulation runner, and a `dashboard/` directory with the standalone HTML
dashboard.

Key files to know about:

| File | Purpose |
|------|---------|
| `src/engine_loop.rs` | The six-phase tick loop — the beating heart of every node (~1045 lines) |
| `src/dht.rs` | Kademlia DHT with latency-weighted k-buckets (~763 lines) |
| `src/transport.rs` | Custom UDP transport with ACK tracking, reliability, and gradient decay |
| `src/forward_pass.rs` | Neural computation: propagate, squash, predict, observe |
| `src/hebbian.rs` | Hebbian STDP learning: weight update, micro-pruning, gradient gossip |
| `src/simulator.rs` | Multi-node orchestration harness with paper mode and CSV export (~1253 lines) |
| `src/observability/` | Metrics registry, Prometheus exporter, OTel tracing, dashboard HTTP server |
| `examples/simulate.rs` | CLI runner that parses `--nodes`, `--duration`, `--paper-mode`, etc. |

## 2. Build

NWP compiles to a single statically linked binary. The release build uses
`opt-level = "z"`, LTO, and stripping for a minimal binary (~500 KB).

```bash
cargo build --release
```

Expected output (your times will vary):

```
Compiling neuron-wire v0.2.0
 Finished `release` profile [optimized + LTO] target(s) in 12.34s
```

If you're short on time, the debug build is faster:

```bash
cargo build
```

> **Troubleshooting:** If compilation fails, ensure you're using Rust edition
> 2021 (rustc 1.80+). Check with `rustc --version`. The project has only 6
> external dependencies — build failures are rare.

## 3. Run the Tests

Verify everything works:

```bash
cargo test
```

Expect output like:

```
running 125 tests
...
test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All tests are deterministic (seeded RNG, no wall-clock assertions). Zero
warnings are enforced across the codebase.

## 4. Run a Quick Simulation

Launch a 3-node, 10-second baseline simulation:

```bash
cargo run --release --example simulate -- --nodes 3 --duration 10
```

You should see the simulator launch three nodes, each running its own engine
loop on a separate thread. Output will include:

```
[SIM] Launching 1 trials of 3 nodes for 10s...
═══════════════════════════════════════════════════
  NEURON-WIRE DHT CONVERGENCE BENCHMARK
  Nodes: 3 × 10s × 1 trial(s)
═══════════════════════════════════════════════════

  Convergence rate:   1/1 (100%)
  Convergence time:   3.00s ± 0.00s  [min=3.0s, max=3.0s]
  Max peers:          2.00 (of 2)
  Avg peers (steady): 1.40
  Bandwidth:          0.0 kbps avg
  Packets/run:        21 total recv
  ✅ ALL TRIALS CONVERGED
```

The key takeaway: **all three nodes discovered each other within ~3 seconds**
and exchanged UDP packets. The DHT routing tables converged — every node knew
every other node.

## 5. Run with the Dashboard

The observability stack includes an **embedded HTTP dashboard** that serves:

- A live topology animation (force-directed graph)
- Chart.js charts for convergence time, bandwidth, and peer discovery
- A full results table
- An SSE stream (`/events`) for real-time updates

To run a simulation with the dashboard enabled, use the `--dashboard` flag
(if supported) or launch the dashboard separately. The dashboard server binds
to `0.0.0.0:9090` by default.

Launch a longer simulation and open the dashboard:

```bash
# Terminal 1: Run the simulation with output
cargo run --release --example simulate -- --nodes 5 --duration 30 --output-dir results/demo

# The dashboard HTML is at dashboard/index.html — open it in a browser
# For the embedded dashboard server, see src/observability/dashboard.rs
```

While the simulation runs, watch the engine logs:

```
[ENGINE] tick=1000 rate=987654Hz rx=3 pkts tx=5 pkts peers=2
[ENGINE] tick=2000 rate=1000000Hz rx=0 pkts tx=0 pkts peers=2
[ENGINE] tick=3000 rate=998000Hz rx=2 pkts tx=2 pkts peers=2
```

These lines tell you:

- **tick=1000** — the node has completed 1000 engine ticks (~1 second at 1ms/tick)
- **rate=987654Hz** — the actual tick rate (should be near 1 MHz)
- **rx=3 pkts** — 3 UDP packets received in this sample
- **peers=2** — this node knows about 2 other nodes (full convergence for a 3-node network)

## 6. Explore the Output

When you pass `--output-dir`, the simulator writes structured CSV files:

```bash
ls results/demo/
```

```
convergence.csv   routing.csv   bandwidth.csv   apoptosis.csv   summary.csv
experiment.toml   metadata.json   raw/
```

View the routing table progression:

```bash
column -t -s, results/demo/routing.csv | head -15
```

Expected format:

```
tick  node_index  peer_count  total_in_table  converged
0     0           0           0               false
100   0           1           1               false
200   0           2           2               true
300   0           2           2               true
```

The `converged` column becomes `true` when the node knows every other node in
the network.

## 7. What Just Happened?

In 15 minutes you:

1. ✅ Built a P2P neural computation fabric from source
2. ✅ Verified 125 passing tests with zero warnings
3. ✅ Launched a 3-node network that discovered itself in ~3 seconds
4. ✅ Saw UDP packet exchange between nodes
5. ✅ Explored CSV output from the deterministic simulator

This is the foundation. In the next tutorial, you'll learn how to build a
multi-node network by hand, understand what each node sees in its routing
table, and troubleshoot discovery issues.

## Source Files Referenced

| File | Lines | What It Does |
|------|-------|-------------|
| [`examples/simulate.rs`](../../examples/simulate.rs) | 1–307 | CLI argument parser and multi-trial runner |
| [`src/simulator.rs`](../../src/simulator.rs) | 1–1253 | Core simulation harness: launch, run, write CSV |
| [`src/engine_loop.rs`](../../src/engine_loop.rs) | 1–1045 | The single-threaded tick loop |
| [`src/observability/dashboard.rs`](../../src/observability/dashboard.rs) | 1–240 | Embedded HTTP server with SSE and JSON APIs |
