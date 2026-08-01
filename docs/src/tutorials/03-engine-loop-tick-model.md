# Tutorial 03: Engine Loop & Tick Model

**Goal:** Understand the six-phase engine loop that drives every NWP node,
the difference between deterministic and real-time execution, and how
backpressure prevents overload.

**Estimated time:** 20 minutes

---

## 1. The Single-Threaded Heart

Every NWP node runs **exactly one OS thread** named `"nwp-engine"`. That thread
owns the UDP socket, the DHT routing table, the neural computation state, and
all periodic maintenance. There is no Tokio. No async runtime. No work-stealing
scheduler.

Why? Three reasons (from `src/engine_loop.rs`, lines 1–50):

```rust
//! ## Why Not Tokio?
//!
//! Tokio on a free-tier VPS (512MB RAM, shared CPU) is overhead we don't need.
//! ...
//! A single-threaded recv_from() loop with a 1ms read timeout gives us:
//!   - Stable tick cadence: ~1ms target per tick (actual timing depends on OS scheduling)
//!   - Zero busy-wait: OS blocks the thread during idle (0% CPU)
//!   - Max throughput: sustained traffic drains as fast as the socket delivers
```

## 2. The Six Phases

Each tick executes exactly six phases in order:

```text
┌────────────────────────────────────────────────────────────┐
│  LOOP (every ~1 ms):                                        │
│                                                              │
│  Phase 1: Drain UDP Socket                                  │
│    recv_from() in a tight loop until WouldBlock             │
│    → parse TransportHeader → ACK tracking                   │
│    → parse NWP header → dispatch by MsgType                 │
│    → events_tx.send(IngressEvent)                           │
│                                                              │
│  Phase 2: Drain Outbound Channel                            │
│    try_recv() on the outbound mpsc channel                  │
│    → send() each OutgoingPacket over UDP                    │
│    → enqueue reliable packets into ReliableQueue            │
│                                                              │
│  Phase 3: Neural Computation (every tick)                   │
│    ForwardPass: leak → propagate → squash → predict → observe│
│    Hebbian: STDP update → micro-prune → gossip              │
│    Neurogenesis: check surprise accumulator → spawn         │
│                                                              │
│  Phase 4: Retransmit (every 10 ticks ~10ms)                 │
│    Scan ReliableQueue for unacked packets                   │
│    Re-send if retries < max and weight > 0.001              │
│    Apply gradient weight decay to all entries               │
│                                                              │
│  Phase 5: Apoptosis / Cleanup (every 1000 ticks ~1s)        │
│    Evict DHT entries: fail_count ≥ 3, latency > 500ms,      │
│      last_seen > 600s                                       │
│    Remove stale orphaned transport frames                   │
│    Death spiral guard: warn if > threshold evicted          │
│                                                              │
│  Phase 6: Yield if Busy                                     │
│    If the tick completed faster than target interval         │
│    (no packets to process), block on recv_from()             │
│    → 0% CPU when idle                                       │
└────────────────────────────────────────────────────────────┘
```

> **Source:** `src/engine_loop.rs` — the main `loop {}` body starts around line 300.
> Each phase is implemented as a separate method (e.g. `phase1_drain_udp()`,
> `phase3_neural_compute()`).

## 3. Tick Timing in Practice

Run a node and watch its tick timing:

```bash
cargo run --release --example simulate -- --nodes 1 --duration 10
```

The engine prints:

```
[ENGINE] tick=1000 rate=987654Hz rx=0 pkts tx=0 pkts peers=0
[ENGINE] tick=2000 rate=1001000Hz rx=0 pkts tx=0 pkts peers=0
```

The `rate` field is the actual tick rate. On Linux x86-64, you'll see
**950 KHz – 1.1 MHz** on an idle single node. On Windows, rates of
**400 KHz – 600 KHz** are normal due to the coarser timer resolution.

### What affects tick rate?

| Factor | Impact | Why |
|--------|--------|-----|
| CPU clock speed | +/– 20% | Faster CPU = shorter Phase 6 sleep |
| Neural computation | –20% to –50% | Forward pass + STDP per tick is ~2 μs of work |
| Heavy packet traffic | –5% to –40% | More recv/send in Phases 1–2 |
| OS timer resolution | –50% (Windows) | `recv_from()` timeout minimum varies by OS |

## 4. Deterministic vs Real-Time Mode

The engine has two fundamentally different execution modes:

### Real-Time Mode (default)

- Uses wall-clock time for `recv_from()` timeout (1 ms)
- `Instant::now()` for timestamp fields in headers
- `std::thread::sleep` or blocking `recv_from()` to maintain tick interval
- Jitter from OS scheduler, timer resolution, and load
- **Use for:** live multi-node networks, real-world deployment

### Deterministic / Paper Mode (`--paper-mode`)

- **Relative tick counters**, not wall-clock timestamps
- All RNG seeded from a fixed seed (default: 42)
- No `Instant::now()` calls in the hot path
- All nodes synchronized via the simulator's coordinated launch
- CSV timestamps are tick numbers, not absolute times
- **Use for:** reproducible experiments, CI validation, paper figures

From `src/simulator.rs`:

```rust
// Paper mode forces:
//   - seed = 42 (if not explicitly set)
//   - no stdout/stderr logging from engine threads
//   - deterministic output CSVs
```

## 5. Backpressure Mechanisms

NWP has three layers of backpressure to prevent overload:

### Layer 1: Non-blocking recv

```rust
// Phase 1 — drain until empty, then move on
loop {
    match socket.recv_from(&mut buf) {
        Ok((len, src)) => process_packet(&buf[..len], src),
        Err(e) if e.kind() == WouldBlock => break,  // ← crucial
        Err(e) => { error!("recv error: {}", e); break; }
    }
}
```

If a burst of packets arrives during Phase 3 (neural compute), they queue in the
socket's receive buffer. The next tick drains them all. No packet is lost due to
busy processing — only if the socket buffer overflows.

### Layer 2: Bounded mpsc Channels

The outbound channel has capacity 10,000:

```rust
let (outbound_tx, outbound_rx) = mpsc::sync_channel::<OutgoingPacket>(10_000);
```

If a subsystem (DHT, Hebbian) tries to enqueue when the channel is full, `send()`
blocks. The engine drains the channel in Phase 2, so as long as the engine keeps
ticking, the channel stays clear.

### Layer 3: Gradient Weight Decay

Reliable packets carry a *gradient weight* that decays exponentially:

```rust
// w(t) = e^(-ln(2) * t / half_life)
// Default half_life = 100 ms
w(t) = e^(-ln(2) * t / 0.1)
```

| Age | Weight | Action |
|-----|--------|--------|
| 0 ms | 1.000 | Full contribution |
| 100 ms | 0.500 | Half weight — retransmit if needed |
| 500 ms | 0.031 | ~3% weight — low priority |
| 1 s | 0.001 | Dropped from queue |

This naturally prioritises fresh information. A gradient that arrives 500 ms
late contributes only 3% — the system doesn't waste bandwidth retransmitting
stale data.

## 6. Walk Through a Single Tick

Let's trace one tick in a converged 3-node network:

**State before tick:** Node A knows Nodes B and C. No pending outbound packets.
Sockets quiet for 50 ms (idle at 0% CPU).

**Phase 1 – Drain UDP:**
- `recv_from()` returns immediately with a PING from Node B
- Parse transport header: `seq=42`, `ack_number=100`
- Update `AckTracker` — record seq 42, advance window
- Parse NWP header: `msg_type=PING(7)`, `body_len=48`
- Dispatch to DHT handler:
  - `handle_ping(sender_id=B, addr=127.0.0.1:9002, ping_seq=17)`
  - DHT upserts Node B into k-bucket (updates EMA latency)
  - Queue a PONG response via `outbound_tx`
- `recv_from()` returns `WouldBlock` — Phase 1 ends

**Phase 2 – Drain Outbound:**
- `try_recv()` gets the PONG packet queued by Phase 1
- Send over UDP: `socket.send_to(pong_bytes, 127.0.0.1:9002)`
- PONG is reliable (Data tier, 3 retries) → also pushed to `ReliableQueue`
- `try_recv()` returns `Empty` — Phase 2 ends

**Phase 3 – Neural Compute:**
- `ForwardPassSystem::tick()`:
  - Leak activations by 0.95
  - Propagate: Σ(weight × pre_activation)
  - Squash via tanh → [-1, 1]
  - Predict → compare to observation → surprise signal
  - Clean orphan activations
- `HebbianLearningSystem::tick()`:
  - Δw = η · pre · post (η = 0.01)
  - Decay: w = (w + Δw) × 0.999
  - Micro-prune: if w < 0.001, remove synapse

**Phase 4 – Retransmit (every 10th tick, skip this one):**
- No-op — counter not yet reached

**Phase 5 – Apoptosis (every 1000th tick, skip this one):**
- No-op — counter not yet reached

**Phase 6 – Yield:**
- Tick completed in ~15 μs (fast, no packets except one)
- Target interval = 1000 μs (1 ms)
- Block `recv_from()` with 985 μs timeout → OS puts thread to sleep
- **0% CPU usage during idle**

## 7. Source Code Map

The engine is implemented entirely in `src/engine_loop.rs` (~1045 lines):

```rust
// EngineLoop struct
pub struct EngineLoop {
    socket: UdpSocket,           // Owns the UDP socket
    outbound_tx: Sender<OutgoingPacket>,
    outbound_rx: Receiver<OutgoingPacket>,
    events_tx: Sender<IngressEvent>,
    dht: DhtHandler,             // Kademlia routing
    forward_pass: ForwardPassSystem,   // Neural prediction
    hebbian: HebbianLearningSystem,     // STDP learning
    neurogenesis: NeurogenesisSystem,   // Neuron birth
    apoptosis: ApoptosisSystem,         // Neuron death
    reliable_queue: ReliableQueue,      // Retransmission
    ack_trackers: HashMap<SocketAddr, AckTracker>,
    stats: EngineStats,
    config: EngineConfig,
    // ...
}

// Core tick method
pub fn tick(&mut self) {
    self.phase1_drain_udp();      // recv and process
    self.phase2_drain_outbound(); // send queued packets
    self.phase3_neural_compute(); // forward + hebbian
    self.phase4_retransmit();     // re-send unacked
    self.phase5_apoptosis();      // prune stale
    self.phase6_yield();          // block if idle
}
```

## Summary

- ✅ The engine is a **single-threaded, 6-phase tick loop** (~1 ms per tick)
- ✅ **Phase 1** drains UDP (non-blocking), **Phase 2** sends outbound, **Phase 3** does neural computation
- ✅ **Phase 4** retransmits stale reliable packets, **Phase 5** prunes dead state
- ✅ **Phase 6** yields to the OS for 0% CPU when idle
- ✅ **Paper mode** uses tick counters instead of wall-clock time for determinism
- ✅ **Three layers of backpressure**: non-blocking recv, bounded channels, gradient decay
- ✅ Single node achieves **~1 MHz tick rate** on Linux, ~400 KHz on Windows

In **Tutorial 04**, you'll take a deep dive into the DHT routing layer —
Kademlia with latency-weighted k-buckets, the bootstrap sequence, and how to
visualise the routing table.

## Source Files Referenced

| File | Lines | Key Content |
|------|-------|-------------|
| [`src/engine_loop.rs`](../../src/engine_loop.rs) | 1–1045 | Full engine implementation, all six phases |
| [`ADR 001`](../../adr/001-single-threaded-engine-loop.md) | 1–77 | Why no async runtime — design rationale |
| [`src/transport.rs`](../../src/transport.rs) | — | `ReliableQueue`, gradient weight decay, ACK tracking |
| [`ARCHITECTURE.md`](../../ARCHITECTURE.md) | §5 | Formal architecture spec with engine loop section |
| [`src/simulator.rs`](../../src/simulator.rs) | — | `SimulationConfig` — paper mode flags |
| [`DEVELOPER_GUIDE.md`](../../DEVELOPER_GUIDE.md) | §4 | Module dependencies and engine loop walkthrough |
