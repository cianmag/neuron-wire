# Developer Guide

A map of the codebase: where things are, how they connect, and how to debug, test, and extend the runtime.

---

## Table of Contents

1. [Codebase Overview](#1-codebase-overview)
2. [Module Dependencies](#2-module-dependencies)
3. [Key Types and Where They Live](#3-key-types-and-where-they-live)
4. [Engine Loop: The Heartbeat](#4-engine-loop-the-heartbeat)
5. [How Data Flows Through the System](#5-how-data-flows-through-the-system)
6. [Adding a New Feature](#6-adding-a-new-feature)
7. [Testing Patterns](#7-testing-patterns)
8. [Debugging Patterns](#8-debugging-patterns)
9. [Performance Profiling](#9-performance-profiling)
10. [Common Pitfalls](#10-common-pitfalls)
11. [Architecture Decision Records](#11-architecture-decision-records)

---

## 1. Codebase Overview

The Rust source lives in `src/`, organized into **17 modules**:

```
src/
├── lib.rs              # Root module: constants (MAGIC, VERSION), type aliases
├── header.rs           # MessageHeader: 16-byte wire header
├── types.rs            # MsgType enum, field offsets, constants (inline mods)
├── flat.rs             # FlatBuffer serialization/deserialization
├── io.rs               # Packet I/O helpers
├── transport.rs        # TransportHeader, AckTracker, ReliableQueue, UdpTransport
├── engine_loop.rs      # EngineLoop: the 6-phase main loop (~913 lines)
├── dht.rs              # DHT handler: k-buckets, bootstrap, latency-weighted routing
├── components.rs       # ECS components: EntityId (256-bit), ActivationMap, SynapseMap
├── forward_pass.rs     # ForwardPassSystem: neural propagation + prediction
├── hebbian.rs          # HebbianLearningSystem: STDP + weight gossip
├── neurogenesis.rs     # NeurogenesisSystem: surprise-driven neuron birth
├── apoptosis.rs        # ApoptosisSystem: timer-based pruning
├── simulator.rs        # Simulation harness: paper mode, multi-trial, CSV export
├── adversary.rs        # Adversarial testing: packet injection, corruption, replay
├── crc.rs              # CRC32 checksum helpers
├── zerocopy.rs         # Zero-copy field access utilities
```

**Deleted orphans** (previously existed as dead files, now removed):
`command.rs`, `consensus.rs`, `data.rs`, `dict.rs`, `network.rs`, `readiness.rs`, `spike.rs`, `udp_transport.rs`.
All their content was already split across `types.rs` (field offsets, inline modules) and `transport.rs` (UdpTransport struct). This cleanup reduces source file count from 25→17 and eliminates confusion.

**Examples:** `examples/simulate.rs` (CLI runner), `examples/zero_copy_demo.rs`.

---

## 2. Module Dependencies

```
                    lib.rs (constants + re-exports)
                   /   |    |    \       \
                 /     |    |     \       \
           header  types  flat  io     crc
              |       |     |      \
              +-------+-----+---- transport
                                   |
                             engine_loop ←─────── the central coordinator
                            /    |    |    \ \
                          /     |    |     \  \
                    dht  forward_pass  hebbian  apoptosis
                     |         |          |
                     |         +---- neurogenesis
                     |
                 simulator    adversary    zerocopy
```

Key observations:
- `engine_loop.rs` is the **single point of integration** — every subsystem is wired through it.
- `transport.rs` + `udp_transport.rs` handle all network I/O; other modules never touch sockets.
- `types.rs` defines the wire format layout — it's the **contract** between sender and receiver.
- `components.rs` (ActivationMap, SynapseMap) is shared by forward_pass, hebbian, neurogenesis, and apoptosis.
- `simulator.rs` and `adversary.rs` are standalone orchestration layers that exercise the engine.

---

## 3. Key Types and Where They Live

| Type | File | Description |
|------|------|-------------|
| `MessageHeader` | `header.rs` | 16-byte wire header (magic, version, type, flags, body_len, CRC) |
| `TransportHeader` | `transport.rs` | 16-byte transport header (seq, ack, bitfield, timestamp) |
| `MsgType` | `types.rs` | Enum: Ping, Pong, Command, Spike, Readiness, Data, Consensus |
| `NodeId` | `dht.rs` | 256-bit peer identifier (kademlia node ID) |
| `EntityId` | `components.rs` | 256-bit neuron identifier (different from NodeId — identifies a neuron within a node) |
| `NeuronId` | `lib.rs` | 64-bit neuron ID alias (`u64`) |
| `EngineConfig` | `engine_loop.rs` | All configuration knobs with sensible defaults |
| `EngineStats` | `engine_loop.rs` | Live counters (shared via `Arc<Mutex<>>`) |
| `OutgoingPacket` | `engine_loop.rs` | Packet enqueued by subsystems for transmission |
| `IngressEvent` | `engine_loop.rs` | Deserialized incoming message dispatched to subsystems |
| `ReliableQueue` | `transport.rs` | Retransmission queue with tiered reliability |
| `AckTracker` | `transport.rs` | Sliding-window ACK state per peer |
| `K Bucket` | `dht.rs` | Kademlia k-bucket (K=20 entries, latency-sorted) |
| `ActivationMap` | `components.rs` | Per-neuron activation buffer |
| `SynapseMap` | `components.rs` | Weighted adjacency for Hebbian STDP |
| `FreshnessConfig` | `dht.rs` | SGA (Sparse Gradient Aging) configuration |

---

## 4. Engine Loop: The Heartbeat

The `EngineLoop` in `engine_loop.rs` is a **single-threaded, non-blocking** loop that runs 6 phases per tick (~1 ms per tick):

```
LOOP:
  Phase 1: Drain UDP     — recv_from() until WouldBlock
  Phase 2: Drain Outbound — send() queued packets
  Phase 3: Neural Compute — ForwardPass + Hebbian STDP (every tick)
  Phase 4: Retransmit    — retry unacked DATA/CONSENSUS (every 10 ticks)
  Phase 5: Apoptosis     — prune stale state (every 1000 ticks)
  Phase 6: Yield         — block on recv_from() if early
```

**Why this matters for development:** Any new subsystem must either:
- (a) Run **synchronously inside one of the 6 phases** (no Tokio tasks, no blocking I/O)
- (b) Communicate with the engine via the **`outbound_tx` / `events_tx` channels**

Pattern (b) is preferred for non-critical-path work. The engine owns the socket — other threads queue packets via `outbound_tx` and the engine drains them in Phase 2.

---

## 5. How Data Flows Through the System

### Inbound (recv → process)

```
UDP recv_from()
  → UdpTransport::recv_packet()     (udp_transport.rs)
    → parse TransportHeader
    → AckTracker::record(seq)
    → parse MessageHeader (validate magic, CRC)
    → dispatch by MsgType:
        Ping/Pong → DhtHandler::process_ping()
        Command   → engine.run_command()
        Spike     → Hebbian::process_spike()
        Readiness → components store readiness
        Data      → Hebbian::process_gradients()
        Consensus → consensus handler
    → events_tx.send(IngressEvent) for non-critical subscribers
```

### Outbound (send → wire)

```
Subsystem (DHT, Hebbian, etc.)
  → outbound_tx.send(OutgoingPacket)
    → engine drains in Phase 2
      → UdpTransport::send_packet()
        → serialize TransportHeader + MessageHeader + FlatBuffer body
        → match reliability:
            BestEffort → send() immediately
            Data/Consensus → ReliableQueue::push() + send()
        → recv_from()
```

---

## 6. Adding a New Feature

### Example: Add a new message type

1. **`types.rs`** — Add variant to `MsgType` enum, add `fixed_size()` arm
2. **`types.rs`** (or a new inline module) — Define field offset constants
3. **`header.rs`** — No changes (header format is generic)
4. **`flat.rs`** — Add serialization/deserialization for the new body layout
5. **`engine_loop.rs`** — Add dispatch arm in the inbound message handler
6. **`transport.rs`** — Choose reliability tier, wire into `ReliableQueue` if needed
7. **Write tests** in an inline `#[cfg(test)] mod tests` block in the new module
8. **`PROTOCOL_SPEC.md`** — Add the new message type to §4
9. **`FOUNDATIONAL_QNA.md`** — If the feature addresses a research question

### Example: Add a new engine phase

1. Insert the phase logic in `engine_loop.rs` in the main `loop {}` body
2. If the phase needs config, add a field to `EngineConfig`
3. Update the architecture diagram in `engine_loop.rs` doc comments
4. Write a test that exercises the phase boundary

### Example: Add a CLI flag to the simulator

1. Parse the flag in `simulator.rs` `parse_args()`
2. Thread it through to `EngineConfig` or `SimConfig`
3. Document in the example header comments and in `REPRODUCIBILITY.md`

---

## 7. Testing Patterns

The project has **77 unit tests** across 16 modules. All tests are inline `#[cfg(test)] mod tests { ... }` blocks — there is no separate `tests/` directory.

### Patterns used

**Deterministic simulation** (preferred for most tests):
```rust
#[test]
fn test_dht_bootstrap() {
    let mut rng = StdRng::seed_from_u64(42);
    // ... components use &mut rng for all random operations
    // Assert deterministic output
}
```

**Network-free unit tests** (for subsystems that don't need the socket):
```rust
#[test]
fn test_weight_decay() {
    let mut h = HebbianLearningSystem::new();
    h.stdp_update(/* ... */);
    assert!(h.weight < 0.5);
}
```

**Adversarial tests** (for protocol robustness):
```rust
// In src/adversary.rs:
fn test_corrupted_packet() { /* inject bad CRC, verify rejection */ }
fn test_replay_attack() { /* replay captured packet, verify detection */ }
fn test_partition() { /* isolate a node, verify reconnection */ }
```

### Running tests

```bash
cargo test                    # All 72 tests
cargo test dht                # DHT subsystem tests only
cargo test -- --show-output   # See stdout from tests
cargo clippy -D warnings      # Zero-warnings enforcement
```

### CI pipeline

The GitHub Actions CI runs: `cargo build --release` → `cargo test` → `cargo clippy -D warnings` → benchmark validation against known-good CSVs.

---

## 8. Debugging Patterns

### Compile-time checks

```bash
# Zero-warnings gate
cargo clippy -D warnings

# Full analysis with nursery lints (occasional false positives)
cargo cliqppy -- -W clippy::nursery

# Check for unsafe blocks (search for all occurrences)
grep -rn "unsafe" src/ | grep -v "#\["
```

### Runtime tracing

```rust
// Add a simple eprintln! guard (the engine runs at ~1ms/tick —
// be careful about I/O volume in hot paths):
if cfg!(feature = "trace") {
    eprintln!("[TRACE] tick={} recv={} sent={}", tick, n_recv, n_sent);
}
```

The simulator's `--paper-mode` automatically suppresses all logging for clean CSV output. Without `--paper-mode`, nodes emit JSONL event logs to `raw/node_NNN.jsonl`, which you can tail:

```bash
cargo run --example simulate -- --nodes 5 --duration 25 --output-dir results/foo
tail -f results/foo/raw/node_000.jsonl | head -50
```

### Panic backtraces

```bash
export RUST_BACKTRACE=1
cargo run --example simulate -- ...
```

### DHT state inspection

The `--output-dir` CSV files include per-tick routing tables:
```bash
# Watch DHT convergence
column -t -s, results/foo/routing.csv | head -20

# Watch bandwidth
column -t -s, results/foo/bandwidth.csv | head -10

# Watch apoptosis events
column -t -s, results/foo/apoptosis.csv | head -10
```

### Adversarial testing

The `--adversary` flag injects failures defined in `src/adversary.rs`:

```bash
cargo run --example simulate -- --nodes 5 --duration 30 --adversary packet-loss:0.1
```

Available adversaral modes: `packet-loss`, `packet-duplication`, `packet-corruption`, `node-partition`, `malicious-node`, `replay-attack`.

---

## 9. Performance Profiling

### CPU profiling

```bash
# Linux: perf record
cargo build --release
perf record --call-graph dwarf ./target/release/neuron-wire --benchmark
perf report

# macOS: Instruments (time profiler)
cargo instruments -t "Time Profiler"

# Simple tick-rate monitoring
cargo run --release --example simulate -- --nodes 50 --duration 10
# Watch "tick_duration_us" in output (target: < 1000 µs)
```

### Memory profiling

```bash
# RSS monitoring
cargo run --release --example simulate -- --nodes 100 --duration 30 &
PID=$!
while kill -0 $PID 2>/dev/null; do
    grep VmRSS /proc/$PID/status
    sleep 1
done

# Valgrind massif (heap profiling)
valgrind --tool=massif ./target/release/neuron-wire --benchmark
```

### Identifying hot loops

The engine's tight loop runs millions of ticks even in short simulations. The most common bottlenecks:

1. **FlatBuffer serialization** in `flat.rs` — hot if many spikes per tick
2. **DHT bucket iteration** in `dht.rs` — hot with 256 buckets × K=20
3. **SynapseMap iteration** in `components.rs` — O(synapses) per tick

Use `#[inline]` annotations on small functions in these paths. The engine already uses `#[inline]` on `TransportHeader` accessors.

---

## 10. Common Pitfalls

**1. Blocking I/O in the engine loop.** The engine runs at ~1 ms per tick. Any blocking call (disk I/O, `std::thread::sleep`, synchronous DNS) stalls the entire runtime. If you need I/O, push it to a separate thread and communicate via the `outbound_tx` channel.

**2. Wall-clock-dependent assertions in tests.** All tests must use seeded RNG and relative ticks, not `Instant::now()`. The CI validates that tests are deterministic — a flaky test fails the pipeline.

**3. FlatBuffer buffer ownership.** `FlatBufferBuilder` owns its buffer until `finished()` is called. Holding a reference across a builder mutation is undefined behavior. When in doubt, use the `#[repr(C)]` transport header which has no lifetime issues.

**4. NodeId vs EntityId.** `NodeId` (32 bytes, `dht.rs`) identifies a **network peer** (a machine running the protocol). `EntityId` (32 bytes, `components.rs`) identifies a **neuron** within the runtime. They are NOT interchangeable, despite both being 256-bit. A single node runs many neurons.

**5. Gradient weight decay in long runs.** With `half_life = 100 ms`, gradients from 500 ms ago contribute only ~3%. If your experiment has large round-trip times (>100 ms), the effective learning rate drops. Adjust `gradient_half_life_ms` in `EngineConfig`.

**6. ACK bitfield wrap-around.** The `AckTracker` handles `u32` sequence number wrap-around, but very long runs (>4 billion packets to a single peer) may hit this. Monitor `seq_wraps` in `EngineStats`.

**7. Orphaned source files.** Files like `command.rs`, `data.rs`, `spike.rs` in `src/` are **not compiled**. Their content was migrated inline into `types.rs`. If you need to modify body layouts, edit `types.rs`, not the orphaned files.

---

## 11. Architecture Decision Records

Every major design decision is documented in `adr/`:

| ADR | Title |
|-----|-------|
| 1 | [Single-Threaded Engine Loop](adr/001-single-threaded-engine-loop.md) |
| 2 | [UDP Transport with Reliability Tiers](adr/002-udp-transport-with-reliability-tiers.md) |
| 3 | [Kademlia DHT with Latency-Weighted k-Buckets](adr/003-kademlia-latency-weighted-kbuckets.md) |
| 4 | [Hebbian STDP Learning (Not Backpropagation)](adr/004-hebbian-stdp-not-backprop.md) |
| 5 | [FlatBuffer Zero-Copy Serialization](adr/005-flatbuffer-zero-copy-serialization.md) |
| 6 | [Sparse Gossip Over Full Mesh](adr/006-sparse-gossip-over-full-mesh.md) |
| 7 | [Deterministic Simulation / Paper Mode](adr/007-deterministic-simulation-paper-mode.md) |
| 8 | [No Persistent Storage (In-Memory Only)](adr/008-no-persistent-storage-in-memory.md) |

If you're making a change that could be controversial or has lasting impact, write an ADR first.

---

*Last updated: 2026-06-28 · Codebase at commit `c54965b`*

[![CI](https://github.com/cianmag/neuron-wire/actions/workflows/ci.yml/badge.svg)](https://github.com/cianmag/neuron-wire/actions/workflows/ci.yml)
[![clippy](https://img.shields.io/badge/clippy-zero%20warnings-brightgreen)]()

| Stat | Value |
|------|-------|
| Tests | 110 |
| Modules | 17 |
| Warnings | 0 (clippy -D warnings) |
| Coverage | ad-hoc (no CI coverage yet) |
