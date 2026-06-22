# Neuron Wire — Architecture Document

> **Decentralized P2P Neural Computing Protocol**  
> Version 0.2.0 — 26 source files, ~6,656 lines of Rust

---

## 1. System Overview

Neuron Wire implements a **peer-to-peer neural computation network** where every node
is simultaneously a neuron in a distributed brain and a router in a Kademlia-style
DHT overlay. The protocol runs over UDP with application-level reliability and
zero-copy FlatBuffer wire encoding.

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │                        NEURON WIRE SYSTEM                            │
 │                                                                      │
 │  ┌──────────────────────────────────────────────────────────────┐    │
 │  │                    EngineLoop (single thread)                  │    │
 │  │  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │    │
 │  │  │   UDP   │  │   DHT    │  │  Neural  │  │  Apoptosis   │   │    │
 │  │  │Transport│◄─┤ Routing  │  │ Compute  │  │  GC + Death  │   │    │
 │  │  │(ACK,   │  │(XOR+DHT) │  │(Forward  │  │  Spiral      │   │    │
 │  │  │bitfield│  │          │  │ Pass +   │  │  Detection   │   │    │
 │  │  │decay)  │  │          │  │ STDP +   │  │              │   │    │
 │  │  └───┬────┘  └────┬─────┘  │Neurogen) │  └──────┬───────┘   │    │
 │  │      │            │        └────┬──────┘         │           │    │
 │  │      │            │             │                │           │    │
 │  │      ▼            ▼             ▼                ▼           │    │
 │  │  ┌────────────────────────────────────────────────────────┐   │    │
 │  │  │              6-Phase Engine Loop Tick                   │   │    │
 │  │  │  recv → drain → neural → retransmit → apoptosis → yield│   │    │
 │  │  └────────────────────────────────────────────────────────┘   │    │
 │  └──────────────────────────────────────────────────────────────┘    │
 │                                                                      │
 │  ┌──────────────────────────┐    ┌───────────────────────────────┐   │
 │  │   outbound_tx (mpsc)     │    │    events_rx (mpsc)           │   │
 │  │   (Sender<OutgoingPacket>)│    │    (Receiver<IngressEvent>)   │   │
 │  └──────────┬───────────────┘    └──────────────┬────────────────┘   │
 │             │                                   │                     │
 │  ┌──────────▼───────────────────────────────────▼──────────────┐     │
 │  │            External Subscribers & Producers                   │    │
 │  │   (DHT handler, Hebbian gossip, Consensus, Cmd Brain)        │    │
 │  └──────────────────────────────────────────────────────────────┘     │
 └──────────────────────────────────────────────────────────────────────┘

 Wire Format (per datagram):
 ┌─────────────────────────────────────────────────────────────────┐
 │  [4B seq] [4B ack] [4B bitfield] [4B ts]  ← 16B Transport Hdr  │
 │  [4B frame_len] [16B MsgHeader] [N B body] ← NWP Frame         │
 └─────────────────────────────────────────────────────────────────┘
```

### Design Tenets

| Principle | Rationale |
|---|---|
| **Single-threaded engine** | No Tokio overhead on 512MB VPS; deterministic 1ms tick; 0% CPU idle |
| **Zero-copy wire** | Wire bytes *are* the data structure — no deserialization, no allocation |
| **UDP-only transport** | No TCP head-of-line blocking; application-level ACK with gradient decay |
| **Reactive Neurogenesis** | Neuron birth is surprise-driven, not timer-based; death is timer-based |
| **Hybrid DHT** | XOR for bucket placement (global reachability) + latency for eviction (performance) |

---

## 2. Engine Loop — The 6-Phase Pipeline

The heart of the system is a **single-threaded, non-blocking event loop**
that owns the UDP socket and orchestrates all subsystems. Each iteration
is a ~1ms tick.

```
  TICK N
    │
    ├── Phase 1: RECV ──────── Drain UDP socket (recv_from, non-blocking)
    │      │                       • Parse 16B transport header (zero-copy)
    │      │                       • Update ACK tracker (bitfield sliding window)
    │      │                       • Process ACK from peer → remove from reliable queue
    │      │                       • Calculate gradient weight from packet age
    │      │                       • Dispatch IngressEvent on events_tx channel
    │      │                       • Drain until WouldBlock or 10,000 packets
    │      ▼
    ├── Phase 2: DRAIN ──────── Drain outbound channel → send()
    │      │                       • try_recv() all queued OutgoingPackets
    │      │                       • Reliable packets → send_reliable() (enqueues for retransmit)
    │      │                       • BestEffort packets → send_best_effort()
    │      ▼
    ├── Phase 3: NEURAL ─────── Forward pass + Hebbian learning (every tick)
    │      │                       • ForwardPassSystem::tick():
    │      │                         LEAK → PROPAGATE → SQUASH → OBSERVE → CLEAN
    │      │                       • HebbianLearningSystem::tick():
    │      │                         STDP → DECAY → MICRO-PRUNE → GOSSIP
    │      ▼
    ├── Phase 4: RETRANSMIT ─── Clean expired + retransmit stale (every 10 ticks)
    │      │                       • cleanup_expired() on reliable queue
    │      │                       • Gradient-weight-based expiry (weight < 0.001)
    │      ▼
    ├── Phase 5: APOPTOSIS ──── GC sweep (every 1,000 ticks ≈ 1s)
    │      │                       • ApoptosisSystem::tick():
    │      │                         1. Collect DHT eviction targets (stale/laggy/dead)
    │      │                         2. Evict from routing table
    │      │                         3. Purge orphaned transport frames (tombstone trap)
    │      │                         4. Expire pending PINGs
    │      │                         5. Clean transport reliable queue
    │      │                       • Death spiral detection (>50 deaths/sweep)
    │      ▼
    └── Phase 6: YIELD ──────── Yield if >100 packets processed this tick
                                    • Prevents CPU saturation on high-throughput
```

### Why Not Tokio?

Tokio's work-stealing scheduler, multi-threaded runtime, and 50+ transitive
crates are overkill for this execution pattern:

```
Pattern:  recv_from() → process → send_to() → repeat
Tick:     ~1ms, deterministic
Idle:     OS blocks on recv_from with 1ms timeout (0% CPU)
Throughput: sustained traffic drains as fast as the socket delivers
```

### EngineConfig Parameters

```rust
pub struct EngineConfig {
    pub bind_addr: String,                    // "0.0.0.0:9000"
    pub tick_interval_ms: u64,                // 1ms
    pub retransmit_interval_ticks: u64,       // every 10 ticks
    pub cleanup_interval_ticks: u64,          // every 1000 ticks (~1s)
    pub max_outbound_queue: usize,            // 10,000 backpressure
    pub recv_buffer_size: usize,              // 65535
    pub gradient_half_life_ms: f32,           // 100ms
    pub local_peers: Vec<SocketAddr>,         // bootstrap peers
}
```

### Channels

| Channel | Type | Direction | Purpose |
|---|---|---|---|
| `outbound_tx` | `Sender<OutgoingPacket>` | → Engine | Any component enqueues NWP frames here |
| `events_rx` | `Receiver<IngressEvent>` | ← Engine | Validated, ACK-tracked messages for subscribers |

```rust
pub struct OutgoingPacket {
    pub payload: Vec<u8>,       // raw NWP frame (header + body)
    pub dst: SocketAddr,
    pub mode: Reliability,       // BestEffort | Data | Consensus
}

pub enum Reliability {
    BestEffort,  // 0 retries (SPIKE, COMMAND, READINESS, GOSSIP)
    Data,        // 3 retries (gradients)
    Consensus,   // 5 retries (voting)
}
```

---

## 3. DHT Routing — Latency-Weighted Hybrid Kademlia

### XOR Distance + Latency Buckets

Standard Kademlia uses XOR distance for bucket placement and time-based eviction.
Neuron Wire uses **XOR for placement** (guarantees global reachability) and
**latency for ranking/eviction** within each bucket.

```
                      NodeId (256-bit XOR space)
  ┌─────────────────────────────────────────────────────────────────┐
  │                                                                 │
  │  255                                   128            0         │
  │  ┌──────┬──────┬──────┬──────┬───┬──────┬──────┬──────┬──────┐  │
  │  │bucket│bucket│bucket│bucket│...│bucket│bucket│bucket│bucket│  │
  │  │ 255  │ 254  │ 253  │ 252  │   │  128 │  127 │  ... │   0  │  │
  │  └──────┴──────┴──────┴──────┴───┴──────┴──────┴──────┴──────┘  │
  │  ▲                                     ▲                        │
  │  │                                     │                        │
  │  Nearest nodes                  Farthest nodes                  │
  │  (bucket 0 = XOR differs       (bucket 255 = XOR differs        │
  │   only in LSB)                  in MSB)                         │
  │                                                                 │
  │  Within each bucket (K=20): sorted by latency (ascending)       │
  │  ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐      │
  │  │ 5ms  │ 12ms │ 15ms │ 22ms │ ...  │ 80ms │ 120ms│ 500ms│      │
  │  │fastest│      │      │      │      │      │      │slowest│     │
  │  └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘      │
  │         ▲                                            ▲          │
  │         │                                            │          │
  │     Eviction-resistant                         First evicted    │
  └─────────────────────────────────────────────────────────────────┘
```

### Bucket Index Computation

```rust
fn bucket_index(local: &NodeId, other: &NodeId) -> Option<u8> {
    let dist = xor_distance(local, other);
    // Find first non-zero byte, then leading-zero bit within it
    // Result: 0=nearest (differ only in LSB), 255=farthest (differ in MSB)
}
```

### K-Bucket Eviction Policy

| Condition | Action |
|---|---|
| Bucket not full | Insert new node, sort by latency |
| Bucket full, new node has lower latency | Evict highest-latency node, insert new node |
| Bucket full, new node has higher latency | Reject new node |
| `fail_count >= 3` | Remove from bucket (Apoptosis sweep) |
| `latency_ms > 500` | Remove from bucket (Apoptosis sweep) |
| `last_seen > 600s` | Remove from bucket (Apoptosis sweep) |

### Bootstrap Priority

```
1. Peer cache file (from previous session on disk)
2. DNS seed resolution (_dht.seeds.<domain>:9000)
3. Hardcoded seed VPS addresses
4. Passive listening (wait for gossip from peers)
```

### DHT Message Types

| Type | Code | Direction | Payload |
|---|---|---|---|
| PING | 7 | Out → In | Sender ID + Addr + Type + Seq |
| PONG | 8 | In → Out | Sender ID + Addr + Type + Seq |
| FIND_NODE | 9 | Out → In | Target ID (32 bytes) |
| NODES | 10 | In → Out | Target ID + K nearest entries |

### Node Types

```rust
pub enum NodeType {
    General   = 0,  Language  = 1,  Reasoning = 2,
    Memory    = 3,  Vision    = 4,  Audio     = 5,
    Consensus = 6,  Gateway   = 7,
}
```

### Lookup Strategy

```
nearest_nodes(target, count):
    collect all entries from all 256 buckets
    sort by XOR distance to target (ascending)
    truncate to `count`
    return

closest_fast(target):
    find the nearest populated bucket to target's XOR position
    within that bucket, return the lowest-latency entry
    spiral outward (alternating +1/-1 offset) if bucket is empty
```

---

## 4. UDP Transport — Gradient-Decay Reliability

### Datagram Layout

```
 ┌─────────────────────────────────────────────────────────────┐
 │  0                   3  4                   7               │
 ├───────────────────────┬─────────────────────────────────────┤
 │    sequence_number     │         ack_number                  │
 ├───────────────────────┼─────────────────────────────────────┤
 │     ack_bitfield       │         timestamp                   │  ← 16B Transport Header
 ├───────────────────────┴─────────────────────────────────────┤
 │   frame_len (u32)     │    NWP Header (16B)  │  NWP Body    │  ← NWP Frame
 └─────────────────────────────────────────────────────────────┘
```

### ACK Bitfield Mechanics

The bitfield acknowledges packets **after** `ack_number`:

```
ack_number = 100

Bit position:  0         1         2       ...     31
Packet:        101       102       103     ...     132
                ↑         ↑          ↑               ↑
             received?  received?  received?      received?

A packet with seq <= ack_number is implicitly acknowledged.
The bitfield extends coverage to 33 packets per datagram.
```

### Sliding Window Advance

```
Window state:  ack_number=100, bitfield=0b1010
               (packets 101=missed, 102=received, 103=missed, 104=received)

Receive seq=101 (fills gap):
  ack_number advances to 102 (bit 0 was set → shift right)
  bitfield = 0b101 → still not contiguous (103 missing)
  ack_number = 102

Receive seq=103:
  bitfield bit for seq=103 is set → ack_number advances to 105
  bitfield → 0
```

### Reliability Policies

| Message Type | Retransmit | Max Retries | Use Case |
|---|---|---|---|
| SPIKE | Never | 0 | Activation signals (fire-and-forget) |
| COMMAND | Never | 0 | Predictive, re-issued on failure |
| READINESS | Never | 0 | Stale after 1 tick |
| DATA (gradients) | Up to 3 | 3 | Weight updates |
| CONSENSUS | Up to 5 | 5 | Distributed voting |
| GOSSIP | Never | 0 | STDP gradient gossip |

### Gradient Weight Decay

```rust
fn calculate_gradient_weight(age_ms: u32, half_life_ms: f32) -> f32 {
    // e^(-ln(2) * dt / half_life)
    // at dt = half_life:    weight = 0.5
    // at dt = 10×half_life: weight ≈ 0.001 → drop
    (-0.69314718 * age_ms as f32 / half_life_ms).exp()
}
```

This means:
- A DATA frame with 100ms half-life that's been pending for 500ms has weight ≈ 0.03
- It will be dropped on the next cleanup sweep
- The sender doesn't need explicit timeout management — gradient decay handles it

### Duplicate Detection

```rust
struct SlidingWindow {
    base: u32,       // oldest sequence in the window
    bits: u64,       // 64-bit bitmask relative to base
}

// check_and_mark(seq):
//    if seq > base + 63: shift window right
//    if seq < base:      return true (too old, already seen)
//    if bit at offset is set: return true (duplicate)
//    else: set bit, return false (new)
```

### Per-Connection State Machine

```rust
struct ConnectionState {
    next_seq: u32,          // monotonic outgoing counter
    ack_number: u32,        // highest contiguous received from peer
    ack_bitfield: u32,      // 32-bit gap mask
    sent_packets: Vec<PendingPacket>,  // un-ACKed packets
    received_seqs: SlidingWindow,      // 64-bit duplicate filter
    rtt_ms: f64,            // exponential moving average RTT
}
```

RTT estimate: `rtt_ms = rtt_ms * 0.9 + sample * 0.1` (EMA, α=0.1)

Retransmit timeout: `max(rtt_ms * 3.0, 100ms)` — adaptive.

---

## 5. Forward Pass System — Neural Computation Pipeline

### Internal 5-Phase Pipeline

```
 ┌─────────────────────────────────────────────────────────────────────┐
 │                  ForwardPassSystem::tick()                            │
 │                                                                      │
 │  ┌─────────┐   ┌──────────┐   ┌────────┐   ┌─────────┐   ┌────────┐ │
 │  │  LEAK   │──▶│PROPAGATE │──▶│ SQUASH │──▶│ OBSERVE │──▶│ CLEAN  │ │
 │  │         │   │          │   │        │   │         │   │        │ │
 │  │Decay all│   │ Σ(w × a) │   │ tanh() │   │compare  │   │remove  │ │
 │  │activns  │   │ for each │   │clamp to│   │pred vs  │   │orphan  │ │
 │  │×0.95    │   │ synapse  │   │[-1, 1] │   │observed │   │neurons │ │
 │  └─────────┘   └──────────┘   └────────┘   └────┬────┘   └────────┘ │
 │                                                   │                  │
 │                                                   ▼                  │
 │                                          ┌─────────────────┐        │
 │                                          │  SURPRISE SIGNAL │        │
 │                                          │  MSE(pred, obs)  │─────▶  │
 │                                          │  → Neurogenesis  │        │
 │                                          └─────────────────┘        │
 └─────────────────────────────────────────────────────────────────────┘
```

### Phase Details

#### 1. LEAK

```rust
for act in activations.values_mut() {
    act.value *= self.activation_leak;  // default: 0.95
}
```

Prevents runaway firing. A neuron with no input decays 5% per tick.
After 100 ticks (100ms) of inactivity: `0.95^100 ≈ 0.006` — effectively zero.

#### 2. PROPAGATE

```rust
for (pre_id, synapse) in synapses.iter() {
    let pre_val = activations[pre_id].value;
    for (i, post_id) in synapse.target_entities.iter().enumerate() {
        *propagation_buffer[post_id] += pre_val * synapse.weights[i];
    }
}
```

Each pre-synaptic neuron's activation is multiplied by its synaptic weight
and added to a propagation buffer for the post-synaptic neuron.

#### 3. SQUASH

```rust
entry.value = tanh(entry.value + raw_value);
```

Tanh squashes any real value to [-1, 1]. Biologically inspired: membrane
potential cannot go to infinity. This also provides a built-in normalization
for the learning dynamics.

#### 4. OBSERVE (Compare → Surprise)

For every incoming observation from the network:

```rust
let surprise = PredictionComponent::new(predicted, observed).surprise();
// surprise = (error²) / (error² + 1.0), normalized to [0, 1]

if surprise > threshold {
    // Feed to NeurogenesisSystem::track_error(surprise)
    // If accumulator breaches threshold → spawn_neuron(causal_inputs)
}
```

The surprise signal has two paths:
- **Known neuron** (we predicted it): compare predicted vs observed value
- **Novel neuron** (unknown EntityId): register it locally, novelty = `|value| × 0.1`

#### 5. CLEAN

```rust
let dead_ids = activations
    .iter()
    .filter(|(id, act)| act.value.abs() < 0.001 && !synapses.contains_key(id))
    .map(|(id, _)| *id)
    .collect();
// Remove neurons with zero activation AND no outgoing synapses
```

### Squashing Function

```rust
pub fn squash_tanh(x: f32) -> f32 {
    x.tanh()  // [-1, 1] range, continuous derivative
}
```

### ForwardPassReport

```rust
pub struct ForwardPassReport {
    pub neurons_updated: usize,
    pub predictions_made: usize,
    pub surprise_events: usize,
    pub total_surprise: f32,
    pub neurons_spawned: usize,
    pub orphans_cleaned: usize,
    pub duration_us: u64,
}
```

---

## 6. Hebbian Learning System — STDP + Micro-Pruning + Gossip

### The Four Phases of a Hebbian Tick

```
For every (post_id, synapse) in the synapse map:
    post_val = activations[post_id]

    ┌─────────────────────────────────────────────────────────────┐
    │ Phase 1: STDP                                                │
    │   Δw = η · pre_activation · post_activation                  │
    │   w += Δw                                                    │
    │   accum_grad += Δw                                           │
    ├─────────────────────────────────────────────────────────────┤
    │ Phase 2: L2 Regularization (weight decay)                    │
    │   w *= λ   (default λ=0.999 → half-life ≈ 693 ticks)        │
    ├─────────────────────────────────────────────────────────────┤
    │ Phase 3: Micro-Pruning                                       │
    │   if w < prune_threshold (0.001):                            │
    │       remove synapse immediately                             │
    ├─────────────────────────────────────────────────────────────┤
    │ Phase 4: Gossip (every 500 ticks ≈ 500ms)                    │
    │   serialize accumulated_gradients → FlatBuffer               │
    │   select 3 gossip targets (cluster-biased random)            │
    │   send on outbound_tx with Reliability::Data                 │
    │   reset accumulators to 0                                    │
    └─────────────────────────────────────────────────────────────┘
```

### Why Micro-Pruning Exists Before Apoptosis

Apoptosis runs every 1s. A weight decaying from 0.1 to 0.001 at λ=0.999
takes ~4,600 ticks (~4.6 seconds) to cross the threshold. That's 4.6 seconds
of wasted CPU and bandwidth on near-zero connections.

Micro-pruning catches them the **instant** they drop below 0.001, freeing
the synapse slot for a new connection in the next Neurogenesis spawn.

### Default Parameters

| Parameter | Symbol | Value | Rationale |
|---|---|---|---|
| Learning rate | η | 0.01 | Slow, stable convergence |
| Weight decay | λ | 0.999 | ~693ms half-life at 1ms ticks |
| Prune threshold | θ | 0.001 | Below this = noise |
| Gossip interval | — | 500 ticks | ~500ms batch window |
| Max gossip targets | — | 3 | Respects free-tier VPS bandwidth |
| Max synapses per gossip | — | 19 | UDP MTU safety (~1400B / 72B per entry) |

### Gossip Target Selection (Cluster-Biased Random)

```
1. Prioritize peers with matching NodeType (same expert cluster)
2. Fall back to random DHT peers
3. Reservoir sample to `MAX_GOSSIP_TARGETS` (3)
4. Send via outbound_tx with Reliability::Data
```

### Gossip Serialization Format

```
 Gossip Packet:
 ┌────────┬──────────┬──────────────────────────────────────────┐
 │ 32B    │ 2B       │ Variable: for each synapse               │
 │ Source │ Count    │ ┌────────┬────────┬────────┬──────────┐ │
 │ ID     │(u16 LE) │ │ 32B    │ 2B     │ for each target:   │ │
 │        │          │ │ PostID │ Count  │ ┌────────┬──────┐ │ │
 │        │          │ │        │(u16 LE)│ │ 32B    │ 4B+4B│ │ │
 │        │          │ │        │        │ │TargetID│ W+G  │ │ │
 │        │          │ │        │        │ └────────┴──────┘ │ │
 │        │          │ └────────┴────────┴────────────────────┘ │
 └────────┴──────────┴──────────────────────────────────────────┘
```

---

## 7. Neurogenesis — Surprise-Driven Neuron Birth

### Why Reactive, Not Timer-Based

Apoptosis is timer-based (Phase 4, every ~1s) because death is predictable:
a connection either is stale or it isn't. Birth is unpredictable — it must
react to the unexpected.

### The Leaky Bucket Accumulator

```
accumulator[t+1] = (accumulator[t] + error_delta) × decay_rate

threshold_breach → spawn neuron → accumulator resets to 0
```

| Property | Value | Effect |
|---|---|---|
| `spawn_threshold` | 0.2 | Accumulator must reach 0.2 to trigger spawn |
| `decay_rate` | 0.99 | Retains 99% per tick; isolates transient noise |
| `MAX_CAUSAL_INPUTS` | 16 | Prevents over-wiring on spawn |

### State Machine

```
IDLE → error arrives → ACCUMULATING → threshold breached
    → SPAWNING (new EntityId + ActivationComponent + SynapseComponent)
    → RESET (accumulator = 0) → IDLE
```

### Surprise Threshold Dynamics

```
Single spike of 0.01:   0.01 × 0.99 = 0.0099 → decays away (no spawn)
Sustained 0.05/tick:    0.05 → 0.054 → 0.058 → ... → 0.20 at tick 6 → SPAWN
After spawn:            accumulator = 0, must learn before next spawn
```

### Spawn Process

```rust
pub fn spawn_neuron(
    activations: &mut ActivationMap,
    synapses: &mut SynapseMap,
    causal_inputs: Vec<EntityId>,   // upstream neurons that caused prediction error
    current_tick: u64,
) -> EntityId {
    // 1. Generate 256-bit cryptographic random ID
    // 2. Create ActivationComponent value=0.0 (resting potential)
    // 3. Wire SynapseComponent to causal_inputs with initial weight=0.1
    // 4. Return new EntityId
}
```

### Error Sources

| Source | Trigger | Delta |
|---|---|---|
| Forward pass prediction | MSE(predicted, observed) | `surprise()` → [0, 1] |
| CRC validation failure | Bad frame CRC | Single tick spike |
| Consensus divergence | Peers disagree | Systemic error |
| DHT lookup failure | Node unreachable | Routing surprise |

Multiple sources accumulate: a network failure + prediction error on the same
tick is more surprising than either alone.

---

## 8. Apoptosis — Programmed Cell Death + Death Spiral Detection

### What Gets Pruned

| Target | Criteria | Sweep | Effect |
|---|---|---|---|
| DHT routing entry | `fail_count >= 3` | Every 1s | Remove from k-bucket |
| DHT routing entry | `latency_ms > 500ms` | Every 1s | Remove from k-bucket |
| DHT routing entry | `last_seen > 600s` (10min) | Every 1s | Remove from k-bucket |
| Pending PING | `age > 10s` (no PONG) | Every 1s | Remove from pending map |
| Reliable DATA frame | `weight < 0.001` (gradient decay) | Every 1s | Remove from send queue |

### Apoptosis Sweep (Phase 4, Every ~1s)

```
1. COLLECT EVICTION TARGETS
   ───────────────────────
   Iterate all DHT routing entries
   Check: fail_count >= 3 || latency > 500ms || last_seen > 600s
   Collect NodeIds into eviction set

2. EVICT FROM DHT
   ──────────────
   For each NodeId in eviction set:
     routing_table.remove(id)

3. PURGE ORPHANED TRANSPORT FRAMES
   ──────────────────────────────
   The Tombstone Trap: frames destined for evicted nodes are dead.
   Call transport.cleanup_expired() → weight-based expiry handles it.

4. EXPIRE PENDING PINGS
   ────────────────────
   For each pending PING older than 10s:
     Remove from pending_pings map
     (future: record failure on associated node → feedback loop)

5. CLEAN TRANSPORT RELIABLE QUEUE
   ─────────────────────────────
   transport.cleanup_expired() handles weight-based removal
```

### The Tombstone Trap

When a DHT node is evicted, any reliable frames (DATA, CONSENSUS) queued
for that destination address become undeliverable. Apoptosis clears them
atomically: we remove the routing entry first, then sweep the reliable queue
for expired frames (gradient weight < 0.001).

### Death Spiral Detection

```rust
pub fn is_death_spiral(&self, report: &ApoptosisReport) -> bool {
    report.total_deaths > 50  // more than 50 deaths per second = panic
}
```

Triggered when >50 nodes are evicted in a single sweep. This indicates:
- Network partition
- Critical seed node failure
- Potential attack (e.g., Sybil nodes being pruned en masse)

### ApoptosisReport

```rust
pub struct ApoptosisReport {
    pub dht_nodes_evicted: usize,
    pub pending_pings_expired: usize,
    pub data_frames_purged: usize,
    pub total_deaths: usize,
    pub sweep_duration_ms: u64,
}
```

---

## 9. ECS Data Model — Entity Component System

### Core Types

```
256-bit EntityId ────────────────────── ActivationComponent
(SHA-256 hash space,      ┌─────────────────────────────┐
 collision probability    │ value: f32                   │
 at 10¹⁸ neurons ≈ 10⁻⁴¹)│ last_updated_tick: u64      │
                          └─────────────────────────────┘
                                  │
                                  ▼
                          SynapseComponent
                    ┌────────────────────────────────────┐
                    │ target_entities: Vec<EntityId>     │ ← causal upstream
                    │ weights: Vec<f32>                  │
                    │ accumulated_gradients: Vec<f32>    │ ← for gossip
                    └────────────────────────────────────┘

                    PredictionComponent (transient)
                    ┌────────────────────────────────────┐
                    │ predicted: f32                     │
                    │ actual: f32                        │
                    │ error: f32 (|predicted - actual|)  │
                    │ error_sq: f32 (error²)              │
                    │ surprise(): f32 = err² / (err² + 1) │
                    └────────────────────────────────────┘
```

### Maps (Type Aliases)

```rust
pub type ActivationMap  = HashMap<EntityId, ActivationComponent>;
pub type SynapseMap     = HashMap<EntityId, SynapseComponent>;
pub type PredictionMap  = HashMap<EntityId, PredictionComponent>;
```

### Direction Convention

In this ECS, `SynapseComponent.target_entities` points **upstream**
(causal direction — which nodes contributed to *this* neuron's prediction
error). The ForwardPassSystem reads activations from upstream nodes and
propagates forward through these connections.

---

## 10. Wire Protocol — NWP v2 Zero-Copy FlatBuffer

### Frame Structure

```
 [0-3]   frame_len: u32        ← Total NWP message size (excludes this field)
 [4-19]  MessageHeader (16B)   ← Magic + Version + Type + Flags + BodyLen + CRC
 [20+N]  Body (N bytes)        ← FlatBuffer-encoded body (fixed + variable regions)
```

### MessageHeader (16 bytes, repr(C))

```rust
#[repr(C)]
pub struct MessageHeader {
    pub magic:      [u8; 4],    // "NWP\0"
    pub version:    u8,         // 2
    pub msg_type:   u8,         // MsgType discriminant
    pub flags:      u16,        // bit flags (COMPRESSED=0x1, ENCRYPTED=0x2, etc.)
    pub body_len:   u32,        // body byte count
    pub header_crc: u32,        // CRC32 of bytes [0..12)
}
```

### Message Types

| Type | Code | Body Size | Purpose |
|---|---|---|---|
| Ping | 0 | 0 | DHT keepalive |
| Pong | 1 | 0 | DHT ping response |
| Command | 2 | 36B fixed + optional | Prediction request to brain regions |
| Spike | 3 | 32B fixed + optional | Activation signal between neurons |
| Readiness | 4 | 20B fixed | Region reports preparation status |
| Data | 5 | 24B fixed + payload | Bulk payload transfer (gradients, etc.) |
| Consensus | 6 | 24B fixed | Distributed voting |

### Zero-Copy Parsing

```rust
// Receiving: cast buffer directly to header
let header = unsafe { &*(buf.as_ptr() as *const MessageHeader) };

// Body access: offset computation into the buffer
let reader = BodyReader::new(body_bytes);
let cmd_id = reader.read_u32(cmd::COMMAND_ID);  // just pointer arithmetic
```

No Vec allocation, no serde deserialization, no parsing step — the wire
bytes *are* the data structure.

### FlatBuffer Body Layout

```rust
struct BodyBuilder {
    fixed: Vec<u8>,   // scalar fields at known offsets
    data: Vec<u8>,    // variable-length strings, vectors
}

// Variable-length data: [4B len][N B data]
// Accessed via relative offsets stored in the fixed region
// Offset = 0 means "not present"
```

---

## 11. Simulator — Orchestration for Reproducible Experiments

### One-Command Paper Reproduction

```bash
cargo run --example simulate -- --paper-mode --nodes 10 --duration 120 \
    --output-dir results/experiment-001
```

### Output Directory Structure

```
results/experiment-001/
├── experiment.toml       # Frozen parameter config (TOML)
├── metadata.json         # Git commit, rustc, platform, timestamps
├── convergence.csv       # Per-tick gradient variance across all nodes
├── bandwidth.csv         # Aggregate bytes in/out per second
├── routing.csv           # DHT peer count per node over time
├── apoptosis.csv         # Death counts per sweep
├── summary.csv           # One-row aggregation per trial
└── raw/                  # Per-node event logs (for deep debugging)
    ├── node_000.jsonl
    ├── node_001.jsonl
    └── ...
```

### SimulationConfig

| Field | Default | Description |
|---|---|---|
| `node_count` | 10 | Number of simulated nodes |
| `duration_secs` | 120 | Wall-clock duration |
| `seed` | 0 (42 in paper mode) | Deterministic RNG seed |
| `tick_interval_ms` | 1 | Engine tick interval |
| `gossip_interval_ticks` | 500 | Hebbian gossip frequency |
| `cleanup_interval_ticks` | 1000 | Apoptosis sweep frequency |
| `max_retries` | 3 | Reliable DATA retries |
| `gradient_half_life_ms` | 100 | Weight decay half-life |

### Convergence Criteria

```rust
pub struct ConvergenceCriteria {
    pub edge_weight_stability_stdev: f32,     // 0.001
    pub edge_weight_stability_window: u32,    // 100 gossip rounds
    pub prediction_error_delta: f32,          // 0.01
    pub prediction_error_window: u32,         // 50 ticks
    pub task_accuracy_plateau_improvement: f32, // 0.001
    pub task_accuracy_plateau_window: u32,    // 10 evaluations
}
```

### Paper Mode

When `--paper-mode` is set:
- All RNG seeded from a single fixed seed (42)
- All parameters frozen into `experiment.toml`
- Expected outputs verified against known-good CSVs
- No non-deterministic logging (timestamps relative, not absolute)

---

## 12. Data Flow Summary

### Ingress Path

```
UDP Socket → recv_from() → parse Transport Header (zero-copy)
    → update AckTracker → process ACK on ReliableQueue
    → calculate gradient_weight(age, half_life)
    → dispatch IngressEvent on events_tx channel
    → subs: DHT handler, Hebbian gossip consumer, Consensus
```

### Egress Path

```
Any component → OutgoingPacket on outbound_tx
    → Engine drains channel → send_best_effort() or send_reliable()
    → Reliable: enqueued in ReliableQueue with seq + half_life
    → UDP socket send_to()
```

### Neural Tick

```
ForwardPassSystem::tick():
    1. Leak all activations × 0.95
    2. Propagate: Σ(weight × pre_activation) into buffer
    3. Squash buffer values through tanh → [-1, 1]
    4. Compare predictions against observations → surprise → Neurogenesis
    5. Clean orphan activations (zero activation, no synapses)

HebbianLearningSystem::tick():
    1. For each synapse: Δw = η · pre · post
    2. Weight decay: w *= λ
    3. Micro-prune: if w < θ, remove
    4. Batch gradients → gossip to ≤3 peers
```

### Apoptosis Sweep (every ~1s)

```
ApoptosisSystem::tick():
    1. Collect DHT eviction targets
    2. Evict from routing table
    3. Purge orphaned transport frames
    4. Expire pending PINGs
    5. Check death spiral (>50 deaths/sweep)
```

---

## 13. Module Map

```
src/
├── lib.rs              # Crate root: re-exports, constants (MAGIC, VERSION)
├── engine_loop.rs      # 6-phase single-threaded engine (860 lines)
├── dht.rs              # XOR-distance Kademlia + latency-weighted buckets (763 lines)
├── transport.rs        # UdpTransport, AckTracker, ReliableQueue (602 lines)
├── udp_transport.rs    # Alternative ConnectionState-based transport (493 lines)
├── forward_pass.rs     # 5-phase neural computation pipeline (385 lines)
├── hebbian.rs          # STDP + micro-pruning + gossip (553 lines)
├── neurogenesis.rs     # Leaky-bucket surprise → neuron spawn (326 lines)
├── apoptosis.rs        # Programmed death + death spiral detection (298 lines)
├── simulator.rs        # Multi-node orchestration + CSV output (527 lines)
├── components.rs       # ECS data types (EntityId, Activation, Synapse) (161 lines)
├── header.rs           # NWP MessageHeader (16B, zero-copy) (139 lines)
├── types.rs            # MsgType enum + field offset constants (193 lines)
├── flat.rs             # FlatBuffer BodyBuilder + BodyReader (234 lines)
├── zerocopy.rs         # MessageRef + BufferPool for zero-copy I/O (186 lines)
├── spike.rs            # SpikeBody (28B fixed, activation signal) (79 lines)
├── data.rs             # DataHeader (24B, bulk payload) (65 lines)
├── command.rs          # CommandBody (32B, prediction instruction) (81 lines)
├── readiness.rs        # ReadinessBody (20B, preparation status) (67 lines)
├── consensus.rs        # ConsensusBody (24B, distributed voting) (57 lines)
├── network.rs          # TCP blocking read/write framing (88 lines)
├── io.rs               # TCP frame I/O (75 lines)
├── crc.rs              # CRC32 wrapper (9 lines)
├── dict.rs             # Dictionary-encoded prediction codes (178 lines)
└── engine_loop.rs      # (also contains spawn_engine + tests)
```

---

> *"A system that grows without pruning is not AGI — it is cancer."*  
> — Apoptosis module comment
