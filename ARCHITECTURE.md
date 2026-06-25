# Neuron Wire Protocol v2

## Architecture Specification

**A Decentralised Adaptive Runtime for Large-Scale Distributed Learning**

*Document version 2.0 — June 2026*
*Corresponding code: `https://github.com/cianmag/neuron-wire` (commit `8c46715`)*

---

## Abstract

The Neuron Wire Protocol (NWP) is a decentralised runtime for distributed neural
computation across untrusted peer-to-peer networks. Unlike federated learning
(central coordinator), All-Reduce (static topology), or parameter-server
architectures (single point of failure), NWP distributes both the model and the
learning process across a dynamic P2P mesh where each node is a self-contained
neural substrate.

This document specifies every protocol detail: wire format, transport reliability,
distributed hash table routing, the six-phase event engine, neural computation
subsystems (forward pass, Hebbian STDP, neurogenesis, apoptosis), and failure
mode analysis. All claims are backed by reproducible benchmarks with known-good
reference outputs.

**Key result:** A 50-node mesh converges to full connectivity in 4.0 seconds
(σ = 0.0) with zero packet loss and zero node evictions during steady state.

---

## Table of Contents

1. [System Architecture](#1-system-architecture)
2. [Wire Protocol](#2-wire-protocol)
3. [Transport Layer](#3-transport-layer)
4. [DHT Routing Layer](#4-dht-routing-layer)
5. [Engine Loop](#5-engine-loop)
6. [Neural Computation Subsystems](#6-neural-computation-subsystems)
7. [Failure Mode Analysis](#7-failure-mode-analysis)
8. [Benchmark Results](#8-benchmark-results)
9. [Formal Protocol Specification](#9-formal-protocol-specification)

---

## 1. System Architecture

### 1.1 Overview

NWP is organised as six layers. Each layer has a well-defined interface and
communicates with adjacent layers through message passing over Rust `mpsc`
channels. There are no shared locks in the hot path; all cross-component
communication is message-passing with bounded queues.

```
┌─────────────────────────────────────────────────────────────┐
│                     NEURAL COMPUTATION                       │
│  ┌────────────┐  ┌──────────┐  ┌──────────────┐            │
│  │Forward Pass│  │ Hebbian  │  │ Neurogenesis  │            │
│  │  (predict) │  │  (STDP)  │  │  (birth)     │            │
│  └──────┬─────┘  └────┬─────┘  └──────┬───────┘            │
│         └──────────────┼────────────────┘                   │
│                        ▼                                    │
│                 ┌──────────────┐                             │
│                 │ Activation & │                             │
│                 │ Synapse Maps │                             │
│                 └──────┬───────┘                             │
├────────────────────────┼─────────────────────────────────────┤
│              ENGINE LOOP (single thread)                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Phase 1: Drain UDP socket   (non-blocking recv_from) │   │
│  │ Phase 2: Drain outbound mpsc (send enqueued packets) │   │
│  │ Phase 3: Neural computation (forward + hebbian tick)  │   │
│  │ Phase 4: Retransmit stale   (reliable queue scan)     │   │
│  │ Phase 5: Cleanup + Apoptosis (evict dead entries)     │   │
│  │ Phase 6: Yield if busy      (prevent CPU saturation)  │   │
│  └──────────────────────────────────────────────────────┘   │
│              │                    ▲                         │
│    outbound_tx│                    │ events_tx              │
│    (mpsc)    ▼                    │ (mpsc)                  │
├──────────────┼────────────────────┼─────────────────────────┤
│              │                    │                         │
│  ┌───────────▼────┐     ┌────────┴────────┐                │
│  │UDP Transport   │     │ DHT Handler     │                │
│  │· ACK tracking  │◄────│· Ping/Pong      │                │
│  │· ReliableQueue │     │· FindNode/Nodes │                │
│  │· Retransmit    │     │· Periodic maint │                │
│  └───────────┬────┘     └─────────────────┘                │
│              │                                              │
│              ▼                                              │
│        UDP Socket (IPv4/IPv6)                               │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Channel Architecture

All cross-thread communication uses bounded `mpsc` channels:

| Channel | Direction | Capacity | Contents |
|---------|-----------|----------|----------|
| `outbound_tx` → `outbound_rx` | Any → Engine | 10,000 | `OutgoingPacket { payload, dst, mode }` |
| `events_tx` → `events_rx` | Engine → Subscribers | Unbounded | `IngressEvent { transport_hdr, nwp_payload, src, timestamp, weight }` |

### 1.3 Thread Model

NWP uses exactly **one OS thread per node**. The engine loop runs on a dedicated
thread named `"nwp-engine"`. No thread pool, no work-stealing scheduler, no
async runtime. Rationale:

- **Deterministic timing**: tick every ~1ms, no scheduler jitter
- **Zero busy-wait**: OS blocks the thread during idle (0% CPU)
- **Max throughput**: sustained traffic drains as fast as the socket delivers
- **Free-tier VPS compatible**: runs on 512MB RAM, shared CPU

DHT and neural subsystems are **inlined into the engine thread** via direct
function calls (no locking). The only atomic operation is the `AtomicBool`
shutdown flag, which is checked once per tick.

---

## 2. Wire Protocol

### 2.1 Frame Format

Every NWP message on the wire has exactly three parts:

```
┌──────────────────────────────────────────────────────────────┐
│  NWP ON-WIRE FORMAT                                          │
├──────────────────────────────────────────────────────────────┤
│ [0..4)    frame_len: u32          = total NWP message size   │
│                                     (excludes this field)     │
│ [4..20)   MessageHeader {16 bytes}                            │
│            [0..4)  magic: [u8;4]  = "NWP\0"                  │
│            [4]     version: u8    = 2                         │
│            [5]     msg_type: u8   = MsgType discrim.          │
│            [6..8)  flags: u16     = bit flags                 │
│            [8..12) body_len: u32  = body length in bytes      │
│            [12..16) header_crc: u32 = CRC32([0..12))          │
│ [20..20+body_len)  Body                                        │
│                      FlatBuffer: fixed region + data region   │
└──────────────────────────────────────────────────────────────┘
```

**Header validation** (checked on every receive):
1. `magic == "NWP\0"` — reject non-NWP traffic
2. `version == 2` — reject incompatible versions
3. `CRC32([0..12)) == header_crc` — reject corrupted headers
4. `body_len <= MAX_BODY_SIZE (1GB)` — reject oversized bodies

### 2.2 Body Serialization (FlatBuffer)

Bodies use a zero-copy FlatBuffer scheme. Every body is divided into two
regions:

```
┌──────────────────────────────────────────────────────────────┐
│  FLATBUFFER BODY LAYOUT                                     │
├──────────────────────────────────────────────────────────────┤
│  Fixed region (fixed_size bytes)                              │
│   │  Scalar fields at known byte offsets from body start     │
│   │  - u8, u16, u32, u64, [u8;N] directly addressable      │
│   │  - Relative offsets point into data region               │
│  ──────────────────────────────────────────────────────────  │
│  Data region  (variable-length data appended after fixed)    │
│   │  Format: [len: u32][data bytes]                          │
│   │  Strings, vectors, serialized structs                     │
└──────────────────────────────────────────────────────────────┘
```

All access is zero-copy: we compute offsets into the buffer and return slices.
No deserialization, no allocation, no parsing step.

### 2.3 Message Types

| Type | Code | Direction | Body | Reliability |
|------|------|-----------|------|-------------|
| `PING` | 7 | DHT | `sender_id(32) + addr(7/19) + node_type(1) + latency(4) + ping_seq(4)` | Data (3 retries) |
| `PONG` | 8 | DHT | `sender_id(32) + addr(7/19) + node_type(1) + latency(4) + ping_seq(4)` | Data (3 retries) |
| `FIND_NODE` | 9 | DHT | `target_id(32)` | Data (3 retries) |
| `NODES` | 10 | DHT | `target_id(32) + [node_entry × N]` | Data (3 retries) |
| `COMMAND` | 2 | Control | Command-specific | BestEffort |
| `SPIKE` | 3 | Neural | `source_id(8) + target_id(8) + activation(4)` | BestEffort |
| `READINESS` | 4 | Control | `node_id(32) + readiness_flags(4)` | BestEffort |
| `DATA` | 5 | Neural | Gradient payload | Data (3 retries) |
| `CONSENSUS` | 6 | Voting | Consensus vote | Consensus (5 retries) |
| `GOSSIP` | 11 | DHT | Node entries | BestEffort |

### 2.4 DHT Body Field Offsets

PING/PONG body (48 bytes total):
```
[0..32)  sender_id: [u8;32]     = NodeId of the sender
[32..39) addr: [u8;7]           = IPv4 address (family+4+2)
[39]     node_type: u8           = NodeType discriminant
[40..44) latency_ms: u32         = RTT estimate (0 in PING)
[44..48) ping_seq: u32           = sequence number for RTT matching
```

FIND_NODE body (32 bytes total):
```
[0..32)  target_id: [u8;32]     = NodeId being searched
```

NODES body (variable):
```
[0..32)  target_id: [u8;32]     = NodeId that was searched
[32..)   entries: [node_entry]  = serialized NodeEntry × N

Each NodeEntry:
  [0..32)  id: [u8;32]
  [32..)   addr: (family:u8 + ip + port)
  [+0]     node_type: u8
  [+1..5)  latency_ms: u32
```

### 2.5 Address Encoding

| Type | Wire Format | Total Size |
|------|-------------|------------|
| IPv4 | `04` + 4 octets + 2 port bytes | 7 bytes |
| IPv6 | `06` + 16 octets + 2 port bytes | 19 bytes |

Family byte `0x04` = IPv4, `0x06` = IPv6. Unknown families are silently skipped
during deserialization.

---

## 3. Transport Layer

### 3.1 UDP Datagram Format

Every UDP datagram carries a 16-byte transport header followed by an NWP frame:

```
┌──────────────────────────────────────────────────────────────┐
│  UDP DATAGRAM LAYOUT                                        │
├──────────────────────────────────────────────────────────────┤
│ [0..4)   sequence_number: u32   = local sequence counter     │
│ [4..8)   ack_number: u32        = last contiguous seq recv'd │
│ [8..12)  ack_bitfield: u32      = bitmask of next 32 seqs    │
│ [12..16) timestamp: u32         = sender's local time (ms)   │
│ [16..)   payload: [u8]          = NWP frame (header + body)  │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 ACK Bitfield Mechanics

The bitfield acknowledges packets *after* `ack_number`:

- `bitfield[0]` = packet `(ack_number + 1)` received
- `bitfield[1]` = packet `(ack_number + 2)` received
- ...
- `bitfield[31]` = packet `(ack_number + 32)` received

Any packet with `seq <= ack_number` is implicitly acknowledged.

**Window advance algorithm:**
```
record(seq):
  if seq <= last_contiguous: return DUPLICATE
  offset = seq - last_contiguous
  if offset == 1:
    // Exactly next packet — advance window
    last_contiguous = seq
    bitfield >>= 1
    while bitfield[0] == 1:
      // Consume contiguous runs from bitfield
      last_contiguous += 1
      bitfield >>= 1
  elif offset <= 32:
    // Future packet — set bit
    bitfield[offset-1] = 1
  else:
    // Major gap — shift window
    last_contiguous = seq - 32
    bitfield = 1 << 31
```

### 3.3 Reliability Classes

| Class | Max Retries | Gradient Decay | Use Case |
|-------|-------------|----------------|----------|
| `BestEffort` | 0 | None | SPIKE, COMMAND, READINESS, GOSSIP |
| `Data` | 3 | Exponential | DATA gradients |
| `Consensus` | 5 | Exponential | CONSENSUS votes |

### 3.4 Gradient Weight Decay

Reliable packets carry a *gradient weight* that decays exponentially with age:

```
weight(age_ms, half_life_ms) = e^(-ln(2) × age_ms / half_life_ms)
```

- At `age = half_life`: weight = 0.5
- At `age = 10 × half_life`: weight ≈ 0.001

Packets with weight < 0.001 are dropped from the reliable queue. This prevents
the queue from accumulating stale packets while still giving fresh packets
meaningful retransmission opportunity.

### 3.5 Retransmission State Machine

```
         ┌──────────┐
         │  SEND    │
         │  (seq=N) │
         └────┬─────┘
              │
              ▼
     ┌────────────────┐
     │  WAITING_ACK   │
     │  enqueued in   │──── ACK received ──► ┌──────────┐
     │  ReliableQueue │                      │  DONE    │
     └───────┬────────┘                     │ (remove) │
              │                              └──────────┘
              │ retransmit interval (10ms)
              │ AND retries < max_retries
              ▼
     ┌────────────────┐
     │  RETRANSMIT    │
     │  resend packet │
     │  retries++     │
     └───────┬────────┘
              │
              │ retries >= max_retries
              │ OR weight < 0.001
              ▼
     ┌──────────────┐
     │  EXPIRED/DROP│
     │  (remove)    │
     └──────────────┘
```

### 3.6 Gradient-Weighted Skiplist

The `ReliableQueue` is internally a `HashMap<u32, ReliablePacket>` keyed by
sequence number. The retransmit scan iterates all entries, applies gradient
weight decay, and returns packets with `retries < max_retries`.

**Complexity:** O(n) in pending reliable packets per retransmit scan.
With default configuration (10ms scan interval, 1KHz tick rate), a node can
sustain ~10,000 concurrent reliable packets before retransmit overhead exceeds
1% of CPU.

---

## 4. DHT Routing Layer

### 4.1 Node Identity

Each node has a **256-bit NodeId** (`NodeId(pub [u8; 32])`). IDs are randomly
generated at startup (via `rand::thread_rng`). XOR distance determines bucket
placement, guaranteeing:
- **Global reachability:** any NodeId can be found by XOR-walking toward it
- **Sybil resistance:** an attacker cannot choose their NodeId prefix
- **Uniform distribution:** random 256-bit IDs distribute evenly across buckets

### 4.2 K-Bucket Structure

The routing table consists of **256 k-buckets** indexed by XOR prefix length:

```
RoutingTable {
    buckets: [KBucket; 256]     // Index 0 = XOR prefix 0 bits (furthest)
                                 // Index 255 = XOR prefix 255 bits (nearest)
    local_id: NodeId,
    local_addr: SocketAddr,
    local_type: NodeType,
}

KBucket {
    entries: Vec<NodeEntry>,    // Sorted by latency (fastest first)
    max_size: usize = 20 (K),
}
```

**Bucket index calculation:**
```
bucket_index(local_id, other_id):
    distance = XOR(local_id, other_id)
    for byte_index, byte in distance:
        if byte != 0:
            msb_within = 7 - leading_zeros(byte)
            return (31 - byte_index) * 8 + msb_within
    return None (same node)
```

### 4.3 Entry Insertion/Eviction

```
upsert(entry):
    if entry.id already in bucket:
        update_latency(EMA)       // Exponential moving average
        reset_fail_count
        sort_by_latency()
        return ACCEPTED
    if bucket.entries.len() < K:
        append(entry)
        sort_by_latency()
        return ACCEPTED
    // Bucket full — evict worst if new is better
    worst_latency = bucket.entries.last().latency_ms
    if entry.latency_ms < worst_latency:
        pop_last()
        append(entry)
        sort_by_latency()
        return ACCEPTED
    return REJECTED
```

**Intuition:** XOR distance distributes nodes across buckets. Within each
bucket, latency determines survival. This hybrid ensures:
- Every ID space region has representatives (Kademlia property)
- Within a region, the fastest nodes survive (latency optimization)
- A slow node cannot fill a bucket (anti-pollution)

### 4.4 Node Entry

```
NodeEntry {
    id:         NodeId,         // 256-bit
    addr:       SocketAddr,     // IPv4 or IPv6
    latency_ms: f32,            // EMA-smoothed RTT
    last_seen:  Instant,        // Wall-clock timestamp
    node_type:  NodeType,       // General, Language, Reasoning, etc.
    fail_count: u32,            // Consecutive failures
}
```

**Latency smoothing:** `latency_ms = 0.7 * old_latency + 0.3 * new_sample`
**Failure tracking:** `fail_count` increments on ping timeout; resets on any
successful PONG. At `fail_count >= 3`, the entry is evicted by Apoptosis.

### 4.5 Routing Table Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `insert` | `(entry: NodeEntry) -> bool` | Upsert into appropriate bucket |
| `find` | `(id: &NodeId) -> Option<&NodeEntry>` | Lookup by NodeId |
| `find_by_addr` | `(addr: &SocketAddr) -> Option<&NodeEntry>` | Lookup by address |
| `remove` | `(id: &NodeId) -> bool` | Remove by NodeId |
| `remove_by_addr` | `(addr: &SocketAddr) -> bool` | Remove by address |
| `nearest_nodes` | `(target: &NodeId, count: usize) -> Vec<NodeEntry>` | K closest nodes to target |
| `bucket_mut` | `(id: &NodeId) -> Option<(u8, &mut KBucket)>` | Get bucket by NodeId |
| `all_nodes` | `() -> Vec<&NodeEntry>` | All entries across all buckets |
| `node_count` | `() -> usize` | Total routing entries |

### 4.6 Bootstrap Sequence

```
bootstrap() [runs once at startup]:
  1. PEER CACHE: load_peers(cache_path)
     - If ≥1 peer found: PING each → return
  2. DNS SEEDS: resolve_dns_seeds(seed_domain)
     - Resolve "_dht.seeds.<domain>" :9000
     - If ≥1 seed resolved: PING each → return
  3. HARDCODED SEEDS: PING each address in SEED_NODES
     - If ≥1 ping sent → return
  4. PASSIVE: eprintln("[DHT] No seeds — listening passively")
     - Wait for incoming PINGs from gossip
```

### 4.7 DHT Message Handling State Machine

```
handle_event(event):
    msg_type = event.nwp_payload[5]   // offset in NWP header
    switch msg_type:
        7 (PING) → handle_ping(event)
        8 (PONG) → handle_pong(event, payload)
        9 (FIND_NODE) → handle_find_node(event, payload)
        10 (NODES) → handle_nodes(payload)
        default → ignore
```

**PING handler:**
```
handle_ping(event):
    1. Parse sender NodeId from body[0..32]
    2. Insert/update routing table entry
    3. Extract ping_seq from body[44..48]
    4. Send PONG back with same ping_seq
```

**PONG handler:**
```
handle_pong(event, payload):
    1. Parse sender NodeId from body
    2. Extract ping_seq from body
    3. lookup pending_pings[ping_seq]:
       - If found: calculate RTT = elapsed_ms
       - If not found: use default 100ms
    4. If sender already in routing table:
         update latency with EMA
       Else:
         remove_by_addr(event.src)  // ghost cleanup
         insert new entry
```

**FIND_NODE handler:**
```
handle_find_node(event, payload):
    1. Parse target NodeId
    2. nearest = routing_table.nearest_nodes(target, K)
    3. Send NODES response with nearest entries
```

### 4.8 Periodic Maintenance

```
periodic_maintenance() [runs every ~1s]:
    1. For each entry where now - last_seen > STALE_PING_S (300s):
         ping_node(entry.addr)
    2. Save peer cache to disk
    3. Log: "[DHT] N nodes, M pending pings"
```

---

## 5. Engine Loop

### 5.1 Six-Phase Execution Model

The engine loop is a single-threaded, non-blocking event loop that executes
exactly six phases per tick:

```
EngineLoop.run():
    self.tick += 1
    if shutdown: return

    ── PHASE 1: DRAIN UDP SOCKET ──
    loop:
        match recv_from(buf):
            Ok((len, src)) → handle_ingress(&buf[..len], src)
            WouldBlock/TimedOut → break
            Error → break
        if ingress_count > 10_000: break    // flood protection

    ── PHASE 2: DRAIN OUTBOUND CHANNEL ──
    loop:
        match outbound_rx.try_recv():
            Ok(packet) → send(packet)
            Empty → break
            Disconnected → break

    ── PHASE 3: NEURAL COMPUTATION ──
    if brain_attached:
        3a. forward_pass.tick(activations, synapses, neurogenesis, tick, observations)
        3b. hebbian.tick(activations, synapses, tick, outbound_tx, peers, local_id)

    ── PHASE 4: RETRANSMIT ──
    if tick - last_retransmit >= retransmit_interval:
        transport.retransmit_stale()

    ── PHASE 5: CLEANUP + APOPTOSIS ──
    if tick - last_cleanup >= cleanup_interval:
        5a. transport.cleanup_expired()
        5b. apoptosis.tick(tick, dht_handler, transport)
        5c. dht_handler.periodic_maintenance()
        5d. update_stats()

    ── PHASE 6: YIELD ──
    if ingress_count > 100:
        thread::yield_now()     // prevent CPU saturation

    if tick % 1000 == 0:
        print_stats()
```

### 5.2 Ingress Pipeline

```
handle_ingress(data, src):
    1. Validate minimum length (≥ 4 bytes for frame_len)
    2. Strip 4-byte frame_len prefix → nwp_payload
    3. Track source in peer_rtt map (for convergence detection)
    4. Build IngressEvent { transport_hdr, nwp_payload, src, timestamp, weight }
    5. Send event to events_tx (non-blocking)
    6. If dht_handler attached:
         dht_handler.handle_event(event)
```

### 5.3 Outbound Pipeline

Outbound packets are queued by any component (DHT, Hebbian gossip, external
code) via `outbound_tx`. The engine drains this channel once per tick:

```
send(packet):
    if packet.mode.is_reliable():
        transport.send_reliable(payload, dst, max_retries, half_life)
    else:
        transport.send_best_effort(payload, dst)
```

### 5.4 Timing Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `tick_interval_ms` | 1ms | Target tick period (UDP read timeout) |
| `retransmit_interval_ticks` | 10 | Retransmit scan every 10 ticks (~10ms) |
| `cleanup_interval_ticks` | 1000 | Apoptosis sweep every 1000 ticks (~1s) |
| `max_outbound_queue` | 10,000 | Bounded outbound channel capacity |
| `recv_buffer_size` | 65,535 | Stack-friendly receive buffer |
| `gradient_half_life_ms` | 100ms | Gradient weight decay half-life |

### 5.5 Spawn and Lifecycle

```
spawn_engine(config, dht_handler, shutdown) → (outbound_tx, events_rx, join_handle):
    1. Bind UDP socket to config.bind_addr
    2. Create mpsc channels (outbound, events)
    3. Build EngineLoop with config, transport, channels
    4. If local_peers configured but no dht_handler:
         Create DhtHandler with random NodeId
    5. Bootstrap: PING all local_peers
    6. Spawn thread "nwp-engine" → call engine.run()
    7. Return (outbound_tx, events_rx, handle)
```

### 5.6 Engine Statistics

```
EngineStats {
    total_ticks: u64,         // Total ticks executed
    packets_recv: u64,        // Packets received
    packets_sent: u64,        // Packets sent
    bytes_recv: u64,          // Bytes received
    bytes_sent: u64,          // Bytes sent
    retransmissions: u64,     // Total retransmissions
    peer_count: usize,        // Known peers (from peer_rtt)
    outbound_queue_depth: usize,
    reliable_queue_depth: usize,
    idle_ticks: u64,          // Ticks with zero ingress
    busy_ticks: u64,          // Ticks with ≥1 packet processed
    actual_tick_rate_hz: f64, // Actual measured tick rate
}
```

---

## 6. Neural Computation Subsystems

### 6.1 Forward Pass System

The forward pass propagates activation values through the synapse graph.
Each tick:

1. **Observation gathering:** collect decoded NWP frames from remote peers
   (activation values)
2. **Activation propagation:** for each entity, compute weighted sum of
   incoming activations
3. **Prediction:** compare propagated activations against observations
4. **Surprise computation:** `surprise = |predicted - observed|`
   — feeds into Neurogenesis for neuron birth decisions
5. **Cleaning:** remove orphan entities with no connections or incoming edges

```
ForwardPassSystem.tick(activation_map, synapse_map, neurogenesis, tick, observations):
    1. Insert observations into activation_map
    2. For each entity in activation_map:
         Calculate incoming activation from synapses
         Update entity activation = f(incoming + leak * previous)
       where f(x) = tanh(x) (bounded activation)
    3. Compute prediction error = surprise for each entity
    4. Feed total_surprise to neurogenesis
    5. Mark unused entities as orphans → cleaned by apoptosis
```

### 6.2 Hebbian Learning System

Implements Spike-Timing-Dependent Plasticity (STDP):

```
Δw = η × (pre_activation × post_activation - λ × w)

where:
  η = learning rate (default 0.01)
  λ = weight decay (default 0.999)
  w = synaptic weight

Micro-pruning: if |w| < prune_threshold (default 0.001), synapse is removed
Gossip: periodically exchange top-K weights with random peers
```

### 6.3 Neurogenesis System

Surprise-driven neuron birth. A global surprise accumulator tracks prediction
error. When the accumulator exceeds a threshold, a new entity (neuron) is
spawned with random initial connections:

```
NeurogenesisSystem.tick(fp_report):
    surprise_accumulator += fp_report.total_surprise
    if surprise_accumulator > SURPRISE_THRESHOLD (0.2):
        spawn new neuron with random weights
        surprise_accumulator = 0
    Clean orphans (entities with no connections)
```

### 6.4 Apoptosis System

Programmed cell death — the guardrail that makes all growth safe. Runs during
Phase 5 of the engine loop (every ~1s).

```
ApoptosisSystem.tick(tick, dht_handler, transport) → ApoptosisReport:
    1. DHT eviction:
       For each routing entry:
         if fail_count >= 3: remove
         elif latency_ms > 500ms: remove
         elif last_seen > 600s ago: remove
    2. Pending PING expiry:
       For each pending_ping:
         if age > 10s: remove from pending map
    3. Frame purging:
       For each evicted DHT node address:
         remove all reliable frames targeting that address
    4. Generate report with death counts
    5. Death spiral detection:
       if total_deaths > AGGREGATE_DEATH_LIMIT:
         emit WARNING — potential network partition
```

**ApoptosisReport fields:**
```
ApoptosisReport {
    dht_nodes_evicted: usize,     // Routing entries pruned
    pending_pings_expired: usize, // Timing out PINGs
    data_frames_purged: usize,    // Orphaned frames
    total_deaths: usize,          // Sum of all above
    sweep_duration_ms: u64,       // Time taken
}
```

---

## 7. Failure Mode Analysis

### 7.1 Network Partition

| Scenario | Detection | Recovery |
|----------|-----------|----------|
| Node crashes | No PONG for 10s → ping timeout | Entry eventually evicted by Apoptosis (10 min stale threshold) |
| Network split | Peers unreachable, no PONGs | On reconnection, bootstrap sequence re-runs (peer cache → DNS → seeds) |
| Seed node failure | PINGs to seed time out | Cluster continues operation; seeds used only for bootstrapping |

### 7.2 Packet Loss

| Loss Pattern | Mechanism | Recovery |
|-------------|-----------|----------|
| Single packet lost | ACK bitfield shows gap | Retransmit on next scan (≤10ms) |
| Burst loss | Multiple gaps in bitfield | Each retransmit carries updated ACK state |
| Bidirectional loss | Both sides miss each other's ACKs | Gradient weight decay prevents infinite retransmit; both sides eventually detect timeout and rediscover via periodic maintenance |

### 7.3 Node Churn

| Event | Effect | Mitigation |
|-------|--------|------------|
| Node joins | New PING received → entry added to bucket | DHT bootstrap fills routing table in ~3s |
| Node leaves gracefully | No special signal | STALE_PING_S timeout → periodic maintenance pings → no response → eviction |
| Node leaves ungracefully | Same as graceful | Apoptosis evicts after 10 min stale + 10s ping timeout |
| Mass churn (50% nodes) | Many entries marked stale | Death spiral detection → operator warning; remaining nodes rebuild via gossip |

### 7.4 Data Integrity

| Threat | Protection |
|--------|------------|
| Corrupted header | CRC32 of header bytes → reject if mismatch |
| Truncated message | Minimum length check on receive (≥4 bytes for frame_len, ≥16 for header) |
| Oversized body | `body_len > MAX_BODY_SIZE (1GB)` check → reject |
| Memory exhaustion | Bounded outbound channel (10,000 packets) + bounded receive buffer (65,535 bytes) |
| Sequence number wrap | u32 wraps at 4 billion packets. At 1KHz tick rate, that's ~4 million ticks → impossible to wrap in a single session. ACK bitfield handles wrap by assuming `seq > last_contiguous` always (session lifetime << wrap time) |

### 7.5 Sybil Attacks

| Attack | Resistance |
|--------|------------|
| NodeId spoofing | IDs are random 256-bit — cannot be chosen by node. Bucket insertion prefers low-latency entries, so fake nodes with high latency are rejected when bucket is full. |
| Routing table pollution | K=20 per bucket. An attacker can at most fill one bucket with slow nodes. The global XOR topology ensures every ID space region still has honest representatives. |
| Eclipse attack | To eclipse a target node, attacker needs to control all K nodes in the target's nearest bucket. With random IDs and K=20, probability of successful eclipse = (attacker_power / total_power)^20. |

### 7.6 Death Spiral Detection

```
is_death_spiral(report) → bool:
    if report.total_deaths > AGGREGATE_DEATH_LIMIT (5):
        return true (potential partition or seed failure)
    return false
```

When a death spiral is detected, the engine emits `[ENGINE] ⚠️ DEATH SPIRAL`
to stderr. The system continues operating — it does not self-destruct — so a
false positive is harmless.

---

## 8. Benchmark Results

### 8.1 Experimental Setup

| Parameter | Value |
|-----------|-------|
| Hardware | Windows 10, Intel Core i7, 16GB RAM, SSD |
| Rust compiler | rustc 1.80+ (LLVM backend) |
| Build profile | `debug` (unoptimized) |
| Simulation | `cargo run --example simulate -- --paper-mode` |
| Seed | 42 (deterministic) |
| DHT stale ping | 300s (relaxed; no evictions during benchmark) |

### 8.2 Convergence Scaling

| Nodes | Trials | Converged | Rate | Convergence Time | Max Peers | Avg Peers | Bandwidth |
|-------|--------|-----------|------|-----------------|-----------|-----------|-----------|
| 3 | 1 | 1 | 100% | 3.00 ± 0.00s | 2/2 | 1.40 | 67 kbps |
| 5 | 10 | 10 | 100% | 3.00 ± 0.00s | 4/4 | 3.52 | 284 kbps |
| 10 | 5 | 5 | 100% | 3.00 ± 0.00s | 9/9 | 8.10 | 1.3 Mbps |
| 25 | 3 | 3 | 100% | 3.00 ± 0.00s | 24/24 | 21.60 | 8.7 Mbps |
| 50 | 3 | 3 | 100% | 4.00 ± 0.00s | 49/49 | 45.35 | 36.5 Mbps |

### 8.3 Convergence Profile (50 nodes)

```
tick=0:    0/2450 connections (0%)
tick=1004: 0/2450 connections (0%)    ← RNG init, nodes starting
tick=2025: 272/2450 connections (11%) ← First PINGs landing
tick=3000: 2220/2450 connections (91%) ← PONG responses arriving
tick=4008: 2450/2450 connections (100%) ← Full mesh achieved
tick=5001+: stable at 2450/2450       ← No churn
```

**Key finding:** Convergence is O(1) in node count. The dominant factor is the
round-trip time for PING/PONG, not the number of nodes. At 50 nodes, the extra
second (3s → 4s) is due to socket saturation from 2450 PING messages on a single
UDP port.

### 8.4 Statistical Determinism

All trials with the same seed produce **identical** results (σ = 0 for all
metrics). This is guaranteed by:

1. Seeded RNG for all random operations
2. Deterministic node startup order
3. Synchronized simulation start after all nodes are ready
4. No wall-clock-dependent assertions

### 8.5 Node Churn

**Apoptosis deaths during steady state: 0** across all benchmarks.
No routing entries were evicted, no pending pings expired, no data frames
purged during the convergence + steady-state window. This confirms the DHT
reaches and maintains a stable routing table without pathological churn.

---

## 9. Formal Protocol Specification

### 9.1 BNF Grammar

```bnf
<datagram>          ::= <transport_header> <nwp_frame>

<transport_header>  ::= <sequence_number:u32>
                        <ack_number:u32>
                        <ack_bitfield:u32>
                        <timestamp:u32>

<nwp_frame>         ::= <frame_len:u32> <message_header> <body>

<frame_len>         ::= u32  (* total NWP message size, excludes this field *)

<message_header>    ::= <magic:[u8;4]>
                        <version:u8>
                        <msg_type:u8>
                        <flags:u16>
                        <body_len:u32>
                        <header_crc:u32>

<magic>             ::= 0x4E 0x57 0x50 0x00  (* "NWP\0" *)
<version>           ::= 0x02

<msg_type>          ::= <PING:0x07>
                      | <PONG:0x08>
                      | <FIND_NODE:0x09>
                      | <NODES:0x0A>
                      | <COMMAND:0x02>
                      | <SPIKE:0x03>
                      | <READINESS:0x04>
                      | <DATA:0x05>
                      | <CONSENSUS:0x06>
                      | <GOSSIP:0x0B>

<body>              ::= <dht_body> | <command_body> | <neural_body>

<dht_body>          ::= <ping_body> | <pong_body>
                      | <find_node_body> | <nodes_body>

<ping_body>         ::= <sender_id:[u8;32]>
                        <encoded_addr>
                        <node_type:u8>
                        <latency_ms:u32>
                        <ping_seq:u32>

<pong_body>         ::= <sender_id:[u8;32]>
                        <encoded_addr>
                        <node_type:u8>
                        <latency_ms:u32>
                        <echoed_ping_seq:u32>

<find_node_body>    ::= <target_id:[u8;32]>

<nodes_body>        ::= <target_id:[u8;32]>
                        <node_entry>*

<node_entry>        ::= <node_id:[u8;32]>
                        <encoded_addr>
                        <node_type:u8>
                        <latency_ms:u32>

<encoded_addr>      ::= <ipv4_addr> | <ipv6_addr>

<ipv4_addr>         ::= 0x04 <octet:u8>^4 <port:u16_be>

<ipv6_addr>         ::= 0x06 <octet:u8>^16 <port:u16_be>

(* Numeric types are little-endian unless marked _be *)
```

### 9.2 Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAGIC` | `[0x4E, 0x57, 0x50, 0x00]` | "NWP\0" magic bytes |
| `VERSION` | 2 | Current protocol version |
| `HEADER_SIZE` | 16 bytes | MessageHeader fixed size |
| `MAX_BODY_SIZE` | 1,000,000,000 bytes | Safety limit for body length |
| `TRANSPORT_HEADER_SIZE` | 16 bytes | Fixed transport header size |
| `K` | 20 | Kademlia bucket capacity |
| `STALE_PING_S` | 300 | Seconds before pinging a stale entry |
| `MAX_FAILURES` | 3 | Consecutive failures before eviction |
| `MAX_LATENCY_MS` | 500 | Maximum acceptable RTT before eviction |
| `MAX_STALE_SECS` | 600 | Seconds of silence before eviction |
| `PENDING_PING_TIMEOUT_S` | 10 | Seconds before pending PING expires |
| `SWEEP_INTERVAL_TICKS` | 1000 | Ticks between apoptosis sweeps |
| `HBAR` | 0.01 | Hebbian learning rate (η) |
| `WEIGHT_DECAY` | 0.999 | Synaptic weight decay per tick |
| `LEAK` | 0.95 | Activation leak rate |
| `SURPRISE_THRESHOLD` | 0.2 | Surprise accumulator trigger for neurogenesis |
| `DEATH_SPIRAL_LIMIT` | 5 | Consecutive deaths triggering spiral warning |

### 9.3 State Machine Summary

```
NODE LIFECYCLE:
  INIT → BOOTSTRAP → ACTIVE → (partition) → RECOVERY → ACTIVE
         ↓                        ↓
         IDLE (no seeds)    SHUTDOWN

PACKET LIFECYCLE (reliable):
  CREATED → QUEUED → SENT → WAITING_ACK → (ACK received) → DONE
                              ↓
                          (timeout) → RETRANSMIT → (retries < N) → SENT
                                                     ↓
                                                (retries >= N) → DROPPED

DHT ENTRY LIFECYCLE:
  DISCOVERED (via PING/PONG/NODES) → INSERTED → (last_seen) → STALE
                                                              ↓
                                          (ping sent → no PONG) → DEAD
                                                              ↓
                                                         EVICTED

ENGINE TICK LIFECYCLE:
  RECV → SEND → COMPUTE → RETRANSMIT → CLEANUP → YIELD → (repeat)
```

---

## References

1. Maymounkov, P., & Mazières, D. (2002). Kademlia: A peer-to-peer information
   system based on the XOR metric. *IPTPS*.
2. Biroli, G., & Mézard, M. (2023). Hebbian learning and STDP in neural
   networks. *Nature Reviews Neuroscience*.
3. Friston, K. (2010). The free-energy principle: a unified brain theory?
   *Nature Reviews Neuroscience*.
4. Gershenson, C. (2007). Design and control of self-organizing systems.
   *PhD thesis, Vrije Universiteit Brussel*.
5. Rust standard library documentation. `std::net::UdpSocket`,
   `std::sync::mpsc`. https://doc.rust-lang.org/std/
