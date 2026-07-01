# Neuron-Wire Examples

> Practical example programs demonstrating the Neuron Wire Protocol (NWP) v2.

All examples are run with `cargo run --example <name>`. They require no external dependencies or network setup — everything runs on localhost.

---

## Available Examples

### 1. `simulate` — Multi-Trial Simulation Runner

The flagship experiment tool. Launches N simulated nodes, runs them for a duration, and reports statistical convergence results.

```bash
# Single trial, 5 nodes, 25 seconds
cargo run --example simulate -- --nodes 5 --duration 25 --seed 42

# Multi-trial statistical benchmark (10 trials)
cargo run --example simulate -- --nodes 10 --duration 60 --trials 10

# Paper mode (deterministic, seed=42)
cargo run --example simulate -- --paper-mode --nodes 20 --duration 120
```

**Output:** `results/experiment_<timestamp>/` directory with per-trial CSVs and a LaTeX-ready summary.

**When to use:** Running experiments, generating paper results, reproducing benchmarks.

### 2. `zero_copy_demo` — FlatBuffer Protocol Pipeline

Demonstrates the full message lifecycle: building COMMAND → READINESS → DATA → CONSENSUS messages, zero-copy parsing via `BodyReader`, CRC integrity verification.

```bash
cargo run --example zero_copy_demo
```

**When to use:** Understanding the wire format, testing protocol changes, learning how messages flow through the system.

### 3. `dashboard` — Live Observability Dashboard

Starts a single node with the live web dashboard on `http://localhost:9090`. See metrics streaming in real-time via Server-Sent Events (SSE).

```bash
cargo run --example dashboard
```

Then open `http://localhost:9090` in your browser.

**When to use:** Visualizing the system in action, monitoring experiments, debugging network behavior.

### 4. `chat` — Interactive P2P Chat

Start multiple nodes and send messages between them. A simple demonstration of the reliability and ordering transport tiers.

```bash
# Terminal 1: Start Alice (port 9000)
cargo run --example chat -- --port 9000 --name alice

# Terminal 2: Start Bob, connecting to Alice
cargo run --example chat -- --port 9001 --name bob --connect 127.0.0.1:9000
```

Type messages and watch them propagate via DHT discovery.

**When to use:** Understanding P2P message flow, testing transport reliability.

### 5. `tune` — Hyperparameter Sweep

Runs a sweep over key configuration parameters (node count, tick rate, stale ping threshold) and reports how each affects convergence time and bandwidth.

```bash
cargo run --example tune
```

**When to use:** Finding optimal configuration for a given deployment scenario, generating hyperparameter sensitivity figures for a paper.

---

## Running All Examples

```bash
# Verify all examples compile
cargo check --examples

# Build all examples (release mode for benchmarks)
cargo build --release --examples
```

## Adding a New Example

1. Create `examples/<name>.rs`
2. Add a `[[example]]` section to `Cargo.toml` if it needs custom features
3. Document it here in `examples/README.md`
4. Verify with `cargo check --examples`

## Output Convention

When your example writes output files, use:

```
results/<example_name>/<timestamp>/
```

This keeps experiment output organized and `.gitignore`-compatible.

## Debugging Examples

```bash
# Run with RUST_LOG for detailed logging
RUST_LOG=debug cargo run --example simulate -- --nodes 5

# Run with backtrace on panic
RUST_BACKTRACE=1 cargo run --example dashboard
```
