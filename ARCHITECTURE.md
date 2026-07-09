# Architecture Specification

> System design for Neuron Wire Protocol (NWP) — peer discovery, transport, neural computation, and simulation.
> See [`PROTOCOL_SPEC.md`](PROTOCOL_SPEC.md) for wire format details.

---

## 1. Overview

NWP is organized as six layers communicating through message passing over bounded Rust `mpsc` channels. No shared locks in the hot path.

```
┌────────────────────────────────────────────────────────────────────┐
│                      EngineLoop (single thread)                     │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐   │
│  │  DHT Routing  │  │  UDP Socket  │  │  Neural Computation      │   │
│  │  (Kademlia +  │◄─┤  (recv/send) │──┤                         │   │
│  │   latency wt)  │  └──────────────┘  │  ┌─────────────────┐   │   │
│  └──────────────┘                      │  │ ForwardPass     │   │   │
│        │                               │  │ (tanh propagate)│   │   │
│        ▼                               │  └─────────────────┘   │   │
│  ┌──────────────┐                      │  ┌─────────────────┐   │   │
│  │ Neurogenesis  │◄──── surprise ──────┤  │ Hebbian (STDP)  │   │   │
│  │ (neuron birth)│                      │  └─────────────────┘   │   │
│  └──────────────┘                      └─────────────────────────┘   │
│        │                                      │                       │
│        ▼                                      ▼                       │
│  ┌──────────────┐                      ┌──────────────┐              │
│  │  Apoptosis    │◄──── prune ─────────┤  Component   │              │
│  │ (neuron death)│                      │  Store (ECS) │              │
│  └──────────────┘                      └──────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

### 1.1 Design Decisions

| Decision | Rationale |
|----------|-----------|
| Single-threaded non-blocking engine | Eliminates scheduler jitter; eliminates 50+ transitive async dependencies; adequate for ~400 KHz–1 MHz on shared CPU (512 MB RAM target) |
| UDP transport | No head-of-line blocking; system selects reliability per message type (Data=3 retries, Consensus=5, BestEffort=0) |
| Kademlia XOR routing | Proven algorithm (BitTorrent, Ethereum); O(log N) lookups; no central registry |
| Sparse gossip (1 peer/tick) | Bounded bandwidth; bias toward high-mutation peers |
| Rust implementation | Memory safety without GC; zero-cost abstractions; single statically linked binary |

---

## 2. Engine Loop

The engine is a single-threaded non-blocking loop owning the UDP socket. Every tick (~1 μs–1 ms, 400 KHz–1 MHz) executes six phases:

| Phase | What | Frequency | Module |
|-------|------|-----------|--------|
| 1. Drain UDP | `recv_from()` until `WouldBlock` | Every tick | `transport.rs` |
| 2. Drain Outbound | `try_recv()` on mpsc channel → `send_to()` | Every tick | `transport.rs` |
| 3. Neural Compute | ForwardPass (leak→propagate→squash→compare→clean) + Hebbian (STDP→micro-prune→gossip) | Every tick | `forward_pass.rs`, `hebbian.rs` |
| 4. Retransmit | Scan reliable queue for unacked DATA/CONSENSUS frames | Every 10 ticks | `transport.rs` |
| 5. Apoptosis | Evict dead DHT nodes, prune orphaned frames, prune stale synapses | Every 1000 ticks | `apoptosis.rs` |
| 6. Yield | If tick completes faster than target interval, OS blocks on `recv_from()` (0% CPU idle) | Every tick | `engine_loop.rs` |

No Tokio. No async runtime. The rationale is documented in [`src/engine_loop.rs`](src/engine_loop.rs): a single-threaded `recv_from()` loop with 1 ms read timeout eliminates scheduler jitter and sustains maximum throughput on free-tier VPS hardware.

---

## 3. DHT Routing

Latency-weighted Kademlia — XOR distance for bucket placement (guarantees global reachability), latency for ranking and eviction within each bucket.

```ascii
┌─────────────────────────────────────────────┐
│             256 K-Buckets                     │
│                                               │
│  Bucket 0           Bucket 1         Bucket 255
│  ┌──────────────┐  ┌──────────────┐  ┌──────┐
│  │ node A 10ms  │  │ node D 22ms  │  │ ...  │
│  │ node B 15ms  │  │ node E 34ms  │  └──────┘
│  │ node C 55ms  │  │ node F 91ms  │
│  └──────────────┘  └──────────────┘
│       ▲                    ▲
│  sorted by latency   sorted by latency
└─────────────────────────────────────────────┘
```

| Parameter | Value |
|-----------|-------|
| K (bucket size) | 20 |
| Buckets | 256 |
| Node ID | 256 bits |
| Latency EMA | `latency = latency × 0.7 + sample × 0.3` |
| Failure threshold | 3 consecutive failures → removal |
| Bootstrap priority | 1) peer cache file, 2) DNS seeds (`_dht.seeds.<domain>`), 3) hardcoded seeds, 4) passive listening |
| Node types | General, Language, Reasoning, Memory, Vision, Audio, Consensus, Gateway |

### Bootstrap Process

1. Load peer cache from disk (if available)
2. DNS seed lookup (`_dht.seeds.zylvon.com`)
3. PING hardcoded seed addresses
4. Iterative FIND_NODE until routing table is populated
5. Periodic maintenance: PING 15 known peers + FIND_NODE 10 unknown targets every 500 ticks

---

## 4. Transport Layer

Custom reliable transport over UDP. Each datagram carries a 16-byte transport header:

```ascii
[ 0- 3] sequence_number  (u32, monotonic per sender)
[ 4- 7] ack_number       (u32, last contiguous seq received)
[ 8-11] ack_bitfield     (u32, bitmask of next 32 packets)
[12-15] timestamp        (u32, sender's local tick in ms)
```

### Reliability Tiers

| Tier | Max Retries | Used For |
|------|-------------|----------|
| BestEffort | 0 | SPIKE, COMMAND, READINESS, GOSSIP |
| Data | 3 | Gradient DATA frames |
| Consensus | 5 | CONSENSUS frames |

### Gradient Weight Decay

Exponential decay on gradient value based on transit time:

```
weight = e^(-ln(2) × Δt / half_life)
```

Default half-life: 100 ms. At Δt = 500 ms, weight ≈ 0.03 (dropped). This naturally prioritises fresh information without explicit timeouts or clock synchronisation.

---

## 5. Neural Computation

### ForwardPassSystem (every tick)

1. **Leak**: decay all activations by 0.95 (prevents runaway firing)
2. **Propagate**: for each neuron, Σ(weight × pre_activation)
3. **Squash**: tanh to [-1, 1]
4. **Compare**: prediction vs. observation → surprise signal → neurogenesis trigger
5. **Clean**: remove orphan activations (no synapses → dead)

### HebbianLearningSystem (every tick)

1. **STDP**: Δw = η · pre_activation · post_activation (η = 0.01)
2. **Decay**: w = (w + Δw) × λ (λ = 0.999)
3. **Micro-prune**: if w < 0.001, remove synapse immediately
4. **Gossip**: every N ticks, serialise gradients into FlatBuffer frame → dispatch to up to 3 DHT peers (cluster-biased)

### Neurogenesis (surprise-driven)

```
accumulator[t+1] = (accumulator[t] + error_delta) × decay_rate
```

- Threshold: 0.2 (20% accumulated error)
- Decay: 0.99 per tick (half-life ≈ 69 ticks)
- Transient noise → decays away, no spawn
- Sustained novelty → accumulator fills → new neuron spawned → accumulator reset
- New neuron: random 256-bit EntityId, synapses initialised to random weights in [-0.1, 0.1]

### Apoptosis (programmed death, every ~1 s)

| Target | Criteria | Effect |
|--------|----------|--------|
| DHT routing entry | fail_count ≥ 3 | Remove from k-bucket |
| DHT routing entry | latency > 500 ms | Remove from k-bucket |
| DHT routing entry | last_seen > 600 s | Remove from k-bucket |
| Pending PING | age > 10 s | Remove from pending map |
| Reliable DATA frame | weight < 0.001 | Remove from send queue |
| Orphaned frame | tied to evicted DHT node | Remove from send queue |

**Death spiral guard**: if single sweep evicts > configurable fraction of routing table, emit warning. Catches network partitions before routing table collapses.

---

## 6. Simulator

Deterministic paper-mode simulation for reproducible experiments:

```bash
cargo run --example simulate -- --paper-mode --nodes 10 --duration 120 \
    --output-dir results/experiment-001
```

When `--paper-mode` is set:
- All RNG seeded from single fixed seed
- All parameters frozen into `experiment.toml`
- Expected outputs verified against known-good CSVs
- Timestamps are relative, not absolute
- Git commit, rustc version, platform captured in `metadata.json`

Output structure:
```
results/experiment-001/
├── experiment.toml       # Frozen parameter config
├── metadata.json         # Git commit, rustc, platform
├── convergence.csv       # Per-tick gradient variance across nodes
├── bandwidth.csv         # Aggregate bytes in/out per second
├── routing.csv           # DHT peer count per node over time
├── apoptosis.csv         # Death counts per sweep
├── summary.csv           # One-row aggregation per trial
└── raw/                  # Per-node event logs
```

---

## 7. Fast DHT Simulators

Two in-process simulators (no sockets) for scaling benchmarks:

### v3 (real node objects)

- `bench-fast.rs` — each node is a real `struct Node { peers: Vec<u32> }`
- Direct message dispatch via shared `Vec<Msg>` buffer
- Proven to 100 K nodes, 100% convergence
- Wall time: 43 s for 100 K nodes
- Avg peers saturates at ~59
- Convergence threshold = max(3 log₂(N), 21)

### v4 Hybrid (active + virtual)

- `bench-fast-v4.rs` — hybrid model for extreme scale
- ACTIVE_MAX = 200 K real nodes, remainder statistical virtual
- Virtual nodes respond with recommended active peers
- MAX_PEERS = 500 k-bucket cap (prevents O(N) memory)
- Fixed bugs:
  - `while i < out.len()` processes msgs generated during delivery
  - Round-robin periodic visits different 20% each round
  - No FIND_NODE relay cascade
- Proven to 1 B nodes, 99.5% convergence, 15.6 s wall time

---

## 8. Module Map

```
src/
├── lib.rs                # Crate root, public exports
├── engine_loop.rs        # Main tick-based engine (1,045 lines)
├── dht.rs                # Kademlia routing (1,209 lines, 7 tests)
├── transport.rs          # UDP transport (642 lines, 10 tests)
├── udp_transport.rs      # UDP implementation detail
├── forward_pass.rs       # Activation propagation (413 lines, 7 tests)
├── hebbian.rs            # STDP learning (683 lines, 8 tests)
├── neurogenesis.rs       # Neuron birth (340 lines, 8 tests)
├── apoptosis.rs          # Neuron death (306 lines, 5 tests)
├── simulator.rs          # Deterministic simulation (1,515 lines)
├── identity.rs           # Ed25519 keys (339 lines)
├── trust.rs              # Sybil-resistant scoring (504 lines)
├── audit.rs              # Hash-chained audit log (534 lines)
├── security.rs           # Security primitives
├── secure_channel.rs     # Noise Protocol-ready channels
├── components/           # EntityId, Neuron/Synapse components (ECS)
├── ml/                   # ML module integration
├── observability/        # Metrics, Prometheus, OTel, Dashboard
│
examples/                 # Runnable examples
tests/                    # Integration, stress, property-based tests
benches/                  # Criterion benchmarks
fuzz/                     # cargo-fuzz targets
docs/                     # mdBook documentation source
ADR/                      # Architecture Decision Records
```

---

## 9. Complexity Analysis

| Domain | Bound | Status |
|--------|-------|--------|
| DHT lookups | O(log N) | Theory — matches empirical convergence time |
| Routing table space | O(K × B) = O(20 × 256) | Constant per node |
| Bootstrap communication | O(N log N) | Empirical — confirmed |
| Steady-state bandwidth | ~O(N) | Empirical — ~2.3 Kbps/node constant |
| Convergence time | O(log N) | Empirical — 7.5 s → 12.5 s from 100 K → 1 B |
| Learning convergence | — | Open — no formal proof yet |

*See [`FORMAL_MODEL.md`](FORMAL_MODEL.md) for full mathematical treatment (1,760 lines, 17 sections).*
