# Neuron Wire Protocol v2

## Architecture Specification

**A Decentralised Adaptive Runtime for Large-Scale Distributed Learning**

*Document version 2.1 — June 2026*
*Corresponding code: `https://github.com/cianmag/neuron-wire` (commit `5dabe67`)*
*Rendered architecture diagram: `architecture-diagram.html` (see [Figure 1](#11-overview))*

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
8. [Complexity Analysis](#8-complexity-analysis)
9. [Benchmark Results](#9-benchmark-results)
10. [Baseline Comparisons](#10-baseline-comparisons)
11. [Formal Protocol Specification](#11-formal-protocol-specification)

---

## 1. System Architecture

### 1.1 Overview

NWP is organised as six layers. Each layer has a well-defined interface and
communicates with adjacent layers through message passing over Rust `mpsc`
channels. There are no shared locks in the hot path; all cross-component
communication is message-passing with bounded queues.

**Figure 1: NWP Layer Architecture** *(also available as rendered HTML at `architecture-diagram.html`)*

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

| Channel | Direction | Capacity | Rust Type | Contents |
|---------|-----------|----------|----------|----------|
| `outbound_tx` → `outbound_rx` | Any → Engine | 10,000 | `Sender<OutgoingPacket>` / `Receiver<OutgoingPacket>` | `OutgoingPacket { payload: Vec<u8>, dst: SocketAddr, mode: Reliability }` |
| `events_tx` → `events_rx` | Engine → Subscribers | Unbounded | `Sender<IngressEvent>` / `Receiver<IngressEvent>` | `IngressEvent { transport_header: TransportHeader, nwp_payload: Vec<u8>, src: SocketAddr, timestamp: u32, weight: f32 }` |

> **Source:** `src/engine_loop.rs` — `EngineLoop::new()` at line ~252 constructs both channels.
> `OutgoingPacket` is defined at line ~156; `IngressEvent` at line ~160; `Reliability` enum at line ~136.

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

| Class | Max Retries | Gradient Decay | Rust Variant | Use Case |
|-------|-------------|----------------|--------------|----------|
| `BestEffort` | 0 | None | `Reliability::BestEffort` | SPIKE, COMMAND, READINESS, GOSSIP |
| `Data` | 3 | Exponential | `Reliability::Data` | DATA gradients |
| `Consensus` | 5 | Exponential | `Reliability::Consensus` | CONSENSUS votes |

> **Source:** `src/engine_loop.rs` line ~136 — `enum Reliability { BestEffort, Data, Consensus }`

### 3.4 Gradient Weight Decay

Reliable packets carry a *gradient weight* that decays exponentially with age:

$$w(t) = e^{-\frac{\ln 2 \cdot t}{t_{1/2}}}$$

where $t$ = age in ms and $t_{1/2}$ = `gradient_half_life_ms` (default 100ms).

| Age | Weight |
|-----|--------|
| $t = t_{1/2}$ | $w = 0.5$ |
| $t = 5 \times t_{1/2}$ | $w \approx 0.031$ |
| $t = 10 \times t_{1/2}$ | $w \approx 0.001$ |

Packets with $w < 0.001$ are dropped from the reliable queue. This prevents
the queue from accumulating stale packets while still giving fresh packets
meaningful retransmission opportunity.

### 3.5 Retransmission State Machine

```mermaid
stateDiagram-v2
    [*] --> SEND : payload enqueued
    SEND --> WAITING_ACK : seq=N sent
    WAITING_ACK --> DONE : ACK received
    WAITING_ACK --> RETRANSMIT : timeout (10ms) ∧ retries < max
    RETRANSMIT --> WAITING_ACK : resend, retries++
    WAITING_ACK --> EXPIRED : retries >= max_retries
    WAITING_ACK --> EXPIRED : gradient weight < 0.001
    EXPIRED --> [*] : removed from queue
    DONE --> [*] : removed from queue
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

Each node has a **256-bit NodeId** (`NodeId(pub [u8; 32])` in `src/dht.rs` line ~16).
IDs are randomly generated at startup (via `rand::thread_rng`). XOR distance determines bucket
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

```mermaid
stateDiagram-v2
    state \"Handle Ingress Event\" as HANDLE_EVENT
    [*] --> HANDLE_EVENT : nwp_payload received

    HANDLE_EVENT --> HANDLE_PING : msg_type == 7 (PING)
    HANDLE_EVENT --> HANDLE_PONG : msg_type == 8 (PONG)
    HANDLE_EVENT --> HANDLE_FIND_NODE : msg_type == 9 (FIND_NODE)
    HANDLE_EVENT --> HANDLE_NODES : msg_type == 10 (NODES)
    HANDLE_EVENT --> IGNORE : otherwise

    state \"handle_ping(event)\" as HANDLE_PING {
        [*] --> Parse_sender_id : body[0..32]
        Parse_sender_id --> Upsert_routing : valid NodeId
        Upsert_routing --> Extract_ping_seq : body[44..48]
        Extract_ping_seq --> Send_PONG : same ping_seq echoed
        Send_PONG --> [*]
    }

    state \"handle_pong(event, payload)\" as HANDLE_PONG {
        [*] --> Parse_sender : body[0..32]
        Parse_sender --> Extract_seq : body[44..48]
        Extract_seq --> Lookup_pending : ping_seq
        Lookup_pending --> Calculate_RTT : pending[seq] found
        Lookup_pending --> Default_100ms : not found
        Calculate_RTT --> Update_Entry
        Default_100ms --> Update_Entry
        Update_Entry --> [*] : EMA latency | insert
    }

    state \"handle_find_node(event, payload)\" as HANDLE_FIND_NODE {
        [*] --> Parse_target : body[0..32]
        Parse_target --> Nearest_K : routing_table.nearest_nodes(target, K)
        Nearest_K --> Send_NODES_response
        Send_NODES_response --> [*]
    }

    state \"handle_nodes(payload)\" as HANDLE_NODES {
        [*] --> Parse_target_id : body[0..32]
        Parse_target_id --> For_each_entry : entries[N]
        For_each_entry --> Upsert_entry : insert/update routing
        Upsert_entry --> For_each_entry : next
        For_each_entry --> [*] : all processed
    }

    state \"ignore\" as IGNORE {
        [*] --> [*]
    }
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

```mermaid
flowchart TD
    TICK["tick += 1"] --> SHUTDOWN{"shutdown flag set?"}
    SHUTDOWN -->|yes| EXIT["return"]
    SHUTDOWN -->|no| PHASE1

    subgraph PHASE1["Phase 1: Drain UDP Socket"]
        direction LR
        RECV["recv_from(buf)"] --> ERR{"Ok?"}
        ERR -->|WouldBlock/TimedOut| P1DONE[""]
        ERR -->|Error| P1DONE
        ERR -->|Ok| INGRESS["handle_ingress(buf, src)"]
        INGRESS --> FLOOD{"ingress_count > 10_000?"}
        FLOOD -->|no| RECV
        FLOOD -->|yes| P1DONE
    end

    PHASE1 --> PHASE2

    subgraph PHASE2["Phase 2: Drain Outbound Queue"]
        TRY_RECV["outbound_rx.try_recv()"] --> MATCH{"Result?"}
        MATCH -->|Ok(pkt)| SEND_PKT["transport.send(pkt)"]
        MATCH -->|Empty| P2DONE[""]
        MATCH -->|Disconnected| P2DONE
        SEND_PKT --> TRY_RECV
    end

    PHASE2 --> PHASE3

    subgraph PHASE3["Phase 3: Neural Computation"]
        BRAIN{"brain_attached?"}
        BRAIN -->|yes| FP["forward_pass.tick()"]
        FP --> HEBBIAN["hebbian.tick()"]
        HEBBIAN --> P3DONE[""]
        BRAIN -->|no| P3DONE
    end

    PHASE3 --> RETRANS_CHECK{"tick - last_retransmit >= interval?"}
    RETRANS_CHECK -->|yes| RETRANS["transport.retransmit_stale()"]
    RETRANS_CHECK -->|no| CLEANUP_CHECK{"tick - last_cleanup >= interval?"}
    RETRANS --> CLEANUP_CHECK

    CLEANUP_CHECK -->|yes| CLEANUP["transport.cleanup_expired()"]
    CLEANUP_CHECK -->|no| PHASE6
    CLEANUP --> APOPTO["apoptosis.tick(tick, dht, transport)"]
    APOPTO --> MAINT["dht.periodic_maintenance()"]
    MAINT --> STATS["update_stats()"]

    subgraph PHASE6["Phase 6: Yield / Stats"]
        BUSY{"ingress_count > 100?"}
        BUSY -->|yes| YIELD["thread::yield_now()"]
        BUSY -->|no| LOG{"tick % 1000 == 0?"}
        LOG -->|yes| PRINT["print_stats()"]
    end

    PHASE6 --> TICK
```

### 5.2 Ingress Pipeline

```rust,ignore
// src/engine_loop.rs — EngineLoop::handle_ingress()
// Rust type: fn handle_ingress(&mut self, data: &[u8], src: SocketAddr)
fn handle_ingress(data: &[u8], src: SocketAddr) {
    1. Validate minimum length (>= TransportHeader::SIZE + 4, i.e. 20 bytes)
    2. Zero-copy parse the TransportHeader from data[0..16):
         seq = TransportHeader::from_bytes(data)
    3. Populate per-peer RTT map:
         peer_rtt.entry(src).or_insert(Instant::now())
    4. Update ACK tracker with received sequence number
    5. Process the ACK this packet carries:
         reliable_queue.process_ack(seq.ack_number, seq.ack_bitfield)
    6. Strip transport header → nwp_frame = data[TransportHeader::SIZE..]
    7. Strip 4-byte frame_len prefix → nwp_payload = nwp_frame[4..]
    8. Build IngressEvent { transport_header, nwp_payload, src, timestamp, weight }
    9. Send event to events_tx (non-blocking)
    10. If dht_handler attached:
         dht_handler.handle_event(event)
}
```

**Key invariants:**
- `handle_ingress` receives the **full UDP datagram** — transport header + NWP frame
- `TransportHeader::from_bytes(data)` is an **unsafe zero-copy cast** (repr(C), no padding)
- The `frame_len` field in the NWP header is validated: `frame_len + 4 <= data.len()`
- Sequence numbers are strictly monotonic per-source; duplicates are dropped by the ACK tracker

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

$$\Delta w = \eta (a_{\text{pre}} \cdot a_{\text{post}} - \lambda \cdot w)$$

where:
- $\eta$ = learning rate (default 0.01)
- $\lambda$ = weight decay (default 0.999)
- $a_{\text{pre}}$, $a_{\text{post}}$ = pre- and post-synaptic activations
- $w$ = synaptic weight

**Micro-pruning:** if $|w| < \tau$ (prune threshold, default 0.001), synapse is removed
**Gossip:** periodically exchange top-K weights with random peers

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

## 8. Complexity Analysis

All complexity bounds are stated for a system of $n$ nodes. Constants assume default configuration (K = 20, tick rate = 1 KHz, packet size = 68 bytes).

### 8.1 Routing Table Memory (per node)

The routing table is partitioned into $b = 256$ k-buckets indexed by XOR prefix length:

$$M_{\text{routing}} = O(K \cdot b) = O(K \cdot \log_2 n)$$

With $K = 20$ and $b = 256$:

$$M_{\text{max}} = K \cdot b \cdot \text{sizeof}(\text{NodeEntry}) = 20 \times 256 \times \sim 80\,\text{B} \approx 400\,\text{KB}$$

In expectation, random 256-bit NodeIds distribute uniformly across all $b$ buckets, so each node holds entries in only $O(\log n)$ distinct buckets. For $n = 10^6$, a node stores entries in roughly $\log_2 10^6 \approx 20$ buckets, with $K$ entries each → $O(K \log n)$ bound holds empirically.

| $n$ | Expected buckets occupied | Expected entries | Memory |
|-----|--------------------------|-----------------|--------|
| $10^1$ | ~4 | 80 | 6 KB |
| $10^3$ | ~10 | 200 | 16 KB |
| $10^6$ | ~20 | 400 | 32 KB |

**Worst case:** all $n$ nodes share the same XOR prefix → one bucket holds all $n$ entries → $O(n)$ memory. This requires adversarial NodeId placement, which is prevented by random 256-bit ID generation (§4.1).

### 8.2 DHT Lookup Complexity

The XOR distance metric induces a binary tree over the ID space. Each lookup step queries the $K$ nearest-known nodes to the target and receives $K$ closer candidates:

$$H_{\text{lookup}} = O\!\left(\log_{K/2} n\right)$$

With $K = 20$, the branching factor is approximately $K/2 = 10$:

| $n$ | Expected hops |
|-----|---------------|
| $50$ | 2 |
| $10^3$ | 3 |
| $10^6$ | 6 |
| $10^9$ | 9 |

**Worst case (degenerate routing table):** if no intermediate buckets have entries, a node may need $O(b) = 256$ iterative hops. This triggers when $\ll K$ nodes exist in the entire network (routing table too sparse), not a concern for $n \geq K$.

In the current implementation, lookup is **implicit**: `handle_find_node` returns $K$ nearest from the local routing table in $O(K)$ time. Full iterative lookup across multiple hops is not implemented because the protocol uses epidemic broadcast (PING/PONG full mesh) rather than iterative routing for convergence. The Kademlia FIND_NODE mechanism is preserved for future scaling beyond ~100 nodes where full mesh becomes infeasible.

### 8.3 Message Complexity

#### 8.3.1 Bootstrap

Each node sends one PING to each peer in its peer cache plus seed nodes:

$$M_{\text{bootstrap}} = O(K \log n) \quad \text{per node}$$

For the full mesh convergence used in benchmarks (every node must discover every other):

$$M_{\text{full-mesh}} = n(n-1) \text{ PINGs} + n(n-1) \text{ PONGs} = \Theta(n^2)$$

| $n$ | Total messages | At 1 KHz drain | Duration |
|-----|---------------|----------------|----------|
| 10 | 180 | 10 ms | 3.0 s (RTT bound) |
| 25 | 1,200 | 60 ms | 3.0 s (RTT bound) |
| 50 | 4,900 | 245 ms | 4.0 s (socket sat.) |

#### 8.3.2 Steady-State Maintenance

Each node pings stale entries every $T_{\text{stale}} = 300$ seconds. Entries are marked stale when `now - last_seen > T_{\text{stale}}`. In a stable network with periodic PING/PONG exchange maintaining freshness, the stale fraction is near zero:

$$M_{\text{maintenance}} = O(n) \quad \text{per sweep, expected} \ll 1 \text{ per node}$$

Measured: **zero** maintenance PINGs during 50-node benchmarks ($\approx 300$s simulation window, overlapping convergence → steady state), confirming $M_{\text{maintenance}} \approx 0$ when all entries are refreshed by convergence traffic.

#### 8.3.3 Gossip

Each gossip interval (default 1000 ticks = 1s), every node selects up to $g = 3$ random peers:

$$M_{\text{gossip}} = g \cdot n = 3n \quad \text{messages per interval}$$

This is $O(n)$ total, $O(1)$ per node.

#### 8.3.4 Minimum Discovery Bound

Every node must receive at least one message from every other node to converge to full connectivity:

$$\Omega_{\text{convergence}} = \Omega(n) \quad \text{messages per node}$$

Since each incoming message carries one sender's NodeId, no protocol can converge in fewer than $n-1$ messages per node. NWP's full-mesh bootstrap achieves $n-1$ in a single flooding round — optimal up to constants.

### 8.4 Bandwidth Complexity

#### 8.4.1 Per-Node Steady State

A converged node sends only gossip and periodic maintenance:

$$B_{\text{steady}} = \frac{g \cdot s_{\text{frame}}}{T_{\text{tick}}} = \frac{3 \times \sim 100\,\text{B}}{1\,\text{s}} \approx 300\,\text{B/s}$$

Zero maintenance bandwidth if gossip refreshes entry timestamps.

#### 8.4.2 Per-Node Convergence Peak

During convergence, each node sends PINGs to all other nodes:

$$B_{\text{peak}} = \frac{n \cdot s_{\text{packet}}}{T_{\text{tick}}} = \frac{n \times 68\,\text{B}}{1\,\text{ms}} = n \times 68\,\text{KB/s}$$

| $n$ | Peak send rate | System total |
|-----|---------------|--------------|
| 10 | 0.68 MB/s | 6.8 MB/s |
| 25 | 1.7 MB/s | 42 MB/s |
| 50 | 3.4 MB/s | 170 MB/s |

These are unconstrained upper bounds. The UDP socket saturates at $\sim 10^4$ packets/tick (§5.1 flood protection), after which excess packets are dropped. Actual measured bandwidth at $n=50$ is 36.5 Mbps ($\approx 4.6$ MB/s system-wide), matching socket-limited throughput.

#### 8.4.3 Reliable Retransmission

Each reliable DATA or CONSENSUS packet generates at most $r = \text{max\_retries}$ retransmissions:

$$B_{\text{reliable}} = (1 + r_{\text{eff}}) \cdot s_{\text{payload}}$$

where $r_{\text{eff}} \leq \text{max\_retries}$ is the actual number of retransmits before ACK. In practice $r_{\text{eff}} \approx 0$ on lossless networks, rising to $r_{\text{eff}} \approx 2$ under 10% packet loss.

### 8.5 Temporal Complexity

#### 8.5.1 Convergence Time

Convergence time is dominated by network round-trip time (RTT), not node count:

$$T_{\text{converge}} = \max\left(\text{RTT}, \frac{n^2 \cdot s_{\text{packet}}}{\nu}\right)$$

where $\nu$ = socket drain rate (packets per tick).

For $n \leq \sqrt{\nu / s_{\text{packet}}}$, the RTT term dominates:

$$n^* = \sqrt{\frac{\nu}{s_{\text{packet}}}} = \sqrt{\frac{10^4}{1}} \approx 100$$

The empirical transition at $n = 25 \to 50$ is earlier ($n^* \approx 35$) because the socket process also spends ticks on outbound drain, neural computation, and retransmission — reducing effective $\nu$ to $\sim 1200$ packets/tick for pure PING/PONG.

| $n$ | Regime | Empirical $T$ | Limiting factor |
|-----|--------|---------------|-----------------|
| $\leq 30$ | RTT-bound | 3.0 s | PING→PONG round-trip |
| $30$–$100$ | Transition | 4.0 s (at $n=50$) | Socket saturation begins |
| $\geq 100$ | Socket-bound | $\propto n^2$ | Single-thread UDP recv |

#### 8.5.2 Apoptosis Sweep

Sweep scans the routing table ($|R|$ entries) and pending PINGs ($|P|$ entries):

$$T_{\text{apoptosis}} = O(|R| + |P|)$$

With $|R| \leq 5120$ and $|P| \ll |R|$, each sweep completes in sub-millisecond on modern hardware. Sweeps run every 1000 ticks (1s).

#### 8.5.3 Retransmit Scan

Scans the reliable queue of size $|Q|$:

$$T_{\text{retransmit}} = O(|Q|)$$

At 10 ms intervals, a $|Q| = 10^4$ queue contributes $<1\%$ CPU overhead. Each scan applies gradient weight decay (one f32 multiply) and checks retry count per entry.

### 8.6 Computational Complexity per Tick

Each tick executes six phases with the following work:

| Phase | Operation | Complexity | Typical $n=50$ |
|-------|-----------|------------|----------------|
| 1 | Ingress drain | $O(p_{\text{recv}})$ | ~50 packets |
| 2 | Outbound drain | $O(p_{\text{send}})$ | ~50 packets |
| 3a | Forward pass | $O(|V| \cdot \bar{d})$ | $|V| \ll 100$, $\bar{d} \approx 3$ |
| 3b | Hebbian STDP | $O(|V| \cdot \bar{d})$ | Same |
| 4 | Retransmit scan | $O(|Q|)$ | $\|Q\| \approx 0$ |
| 5 | Apoptosis sweep | $O(|R| + |P|)$ | Every 1000 ticks |
| 6 | Yield | $O(1)$ | Conditional |

Total work per tick:

$$W_{\text{tick}} = O\!\left(p_{\text{recv}} + p_{\text{send}} + |V| \cdot \bar{d} + \frac{|R|}{1000}\right)$$

For the neural computation regime ($|V| \leq 10^3$, $\bar{d} \leq 10$), each tick completes in under $100\,\mu\text{s}$, leaving $>90\%$ of the 1 ms budget idle.

### 8.7 Complexity Summary

| Dimension | Complexity | Effective bound | Empirical validation |
|-----------|-----------|----------------|---------------------|
| Routing memory (per node) | $O(K \log n)$ | $\leq 400$ KB at $n=10^6$ | 45 entries at $n=50$ = 3.6 KB |
| Lookup hops | $O(\log_{K/2} n)$ | $\leq 6$ hops at $n=10^6$ | Single-hop (full mesh) at $n\leq 50$ |
| Bootstrap messages (total) | $\Theta(n^2)$ | $2n(n-1)$ | 4,900 at $n=50$ |
| Maintenance (per sweep) | $O(n)$ expected | $\approx 0$ in steady state | Zero at $n=50$ |
| Convergence time | $O(\text{RTT} + n^2/\nu)$ | $\leq 4$ s at $n=50$ | 4.0 s ($\sigma = 0$) |
| Reliable queue (per op) | $O(1)$ amortized | — | — |
| Retransmit scan | $O(|Q|)$ | $\|Q\| \leq 10^4$ | $\|Q\| \approx 0$ on lossless |
| Tick computation | $O(p + \|V\|\bar{d})$ | $< 100\,\mu\text{s}$ per tick | 1 ms budget, $>90\%$ idle |
| Apoptosis sweep | $O(|R|)$ | $|R| \leq 5120$ | Sub-ms |
| Sybil eclipse resistance | $\Theta\!\left((p/t)^K\right)$ | $K=20$ | — |

where $p$ = attacker-controlled fraction of total nodes, $t$ = honest fraction.

---

## 9. Benchmark Results

### 9.1 Experimental Setup

| Parameter | Value |
|-----------|-------|
| Hardware | Windows 10, Intel Core i7, 16GB RAM, SSD |
| Rust compiler | rustc 1.80+ (LLVM backend) |
| Build profile | `debug` (unoptimized) |
| Simulation | `cargo run --example simulate -- --paper-mode` |
| Seed | 42 (deterministic) |
| DHT stale ping | 300s (relaxed; no evictions during benchmark) |

### 9.2 Convergence Scaling

| Nodes | Trials | Converged | Rate | Convergence Time | Max Peers | Avg Peers | Bandwidth |
|-------|--------|-----------|------|-----------------|-----------|-----------|-----------|
| 3 | 1 | 1 | 100% | 3.00 ± 0.00s | 2/2 | 1.40 | 67 kbps |
| 5 | 10 | 10 | 100% | 3.00 ± 0.00s | 4/4 | 3.52 | 284 kbps |
| 10 | 5 | 5 | 100% | 3.00 ± 0.00s | 9/9 | 8.10 | 1.3 Mbps |
| 25 | 3 | 3 | 100% | 3.00 ± 0.00s | 24/24 | 21.60 | 8.7 Mbps |
| 50 | 3 | 3 | 100% | 4.00 ± 0.00s | 49/49 | 45.35 | 36.5 Mbps |

### 9.3 Convergence Profile (50 nodes)

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

### 9.4 Statistical Determinism

All trials with the same seed produce **identical** results (σ = 0 for all
metrics). This is guaranteed by:

1. Seeded RNG for all random operations
2. Deterministic node startup order
3. Synchronized simulation start after all nodes are ready
4. No wall-clock-dependent assertions

### 9.5 Node Churn

**Apoptosis deaths during steady state: 0** across all benchmarks.
No routing entries were evicted, no pending pings expired, no data frames
purged during the convergence + steady-state window. This confirms the DHT
reaches and maintains a stable routing table without pathological churn.

---

## 10. Baseline Comparisons

Comparative analysis against five standard distributed system architectures.
All metrics are analytical (§8) or empirically measured at $n=50$ (§9).

### 10.1 Baseline Definitions

| Baseline | Model | Description |
|----------|-------|-------------|
| **Random routing** | Unstructured P2P | Each node maintains a random subset of peers. Discovery proceeds by random probe: node $i$ picks $k$ random known peers, asks for their contacts, and iterates. No XOR distance, no k-buckets. |
| **Centralized coordinator** | Client-server | A single coordinator node maintains the full membership list. Every joining node registers with the coordinator; the coordinator broadcasts membership updates to all nodes. |
| **Standard gossip** | Epidemic broadcast | Each node periodically contacts $b$ random peers and exchanges its full routing table. No structured bucket hierarchy — epidemics rely on fanout for coverage. |
| **Federated averaging** | Centralized ML | Nodes train locally; a central server aggregates model weights each round. Only the gradient/model delta is communicated, but full mesh discovery is not required — nodes only talk to the server. |
| **Static graph** | Fixed topology | A $d$-regular graph is pre-configured. No peer discovery occurs; nodes communicate only with their $d$ fixed neighbors. Failures are not repaired. |

### 10.2 Comparison Dimensions

#### Convergence to Full Mesh

The metric: time until every node knows every other node's address.

| Baseline | Time to full mesh | Derivation | At $n=50$ |
|----------|-------------------|------------|-----------|
| **NWP (this work)** | $O(\text{RTT} + n^2/\nu)$ | PING/PONG flood with Kademlia routing | **4.0 s** |
| Random routing | $O(n \log n \cdot \text{RTT})$ | Coupon collector: each random probe has $1/n$ chance of finding new node; need $n$ discoveries per node | $\approx 50 \cdot \ln 50 \cdot \text{RTT} \approx 185 \, \text{RTT} \approx 55 \, \text{s}$ |
| Centralized coordinator | $O(1)$ rounds | One registration + one broadcast round | **2 RTT ≈ 6 ms** |
| Standard gossip | $O(\log n \cdot \text{RTT})$ | Epidemic spread covers population in $\log_{b+1} n$ rounds | $\log_4 50 \cdot \text{RTT} \approx 3 \, \text{RTT} \approx 9 \, \text{ms}$ |
| Federated averaging | $n/a$ | No mesh required — only server contact | — |
| Static graph | $n/a$ | No discovery; topology is fixed | — |

Centralized coordinator is fastest, but at the cost of a single point of failure and $O(n^2)$ server bandwidth. NWP is competitive when RTT dominates ($n < 35$, §8.5.1) and degrades gracefully to socket-bound $O(n^2/\nu)$ beyond.

#### Message Complexity (Total Network)

| Baseline | Messages to converge | At $n=50$ |
|----------|---------------------|-----------|
| **NWP (this work)** | $\Theta(n^2)$ PING + PONG | **4,900** |
| Random routing | $O(n^2 \log n)$ | $\approx 50^2 \cdot \ln 50 \approx 9,750$ |
| Centralized coordinator | $O(n)$ | $n$ registration + $n$ broadcast = 100 |
| Standard gossip | $O(n \log n)$ | $50 \cdot \log_4 50 \cdot b \approx 50 \cdot 3 \cdot 3 = 450$ |
| Federated averaging | $O(n)$ per round | 100 per round |
| Static graph | $0$ | No discovery messages |

NWP sends more discovery messages than centralized or gossip approaches because it implements a full-mesh broadcast rather than a spanning tree. The trade-off is **fault tolerance**: NWP has no single point of failure and nodes require no configuration.

#### Steady-State Bandwidth (Per Node)

| Baseline | Bandwidth | At $n=50$ | At $n=10^3$ |
|----------|-----------|-----------|-------------|
| **NWP (this work)** | $\sim 300\,\text{B/s}$ | **300 B/s** | **300 B/s** |
| Random routing | $O(k \cdot s_{\text{entry}})$ periodic probe | $\sim 1\,\text{KB/s}$ | $\sim 10\,\text{KB/s}$ |
| Centralized coordinator | $O(n)$ heartbeat to server | $\sim 50\,\text{B/s}$ | $\sim 1\,\text{KB/s}$ |
| Standard gossip | $O(b \cdot s_{\text{full}})$ per interval | $\sim 10\,\text{KB/s}$ | $\sim 10\,\text{KB/s}$ |
| Federated averaging | $O(\text{model})$ per round | $\sim 1\,\text{MB/s}$ (1M param) | $\sim 1\,\text{MB/s}$ |
| Static graph | $O(d \cdot s_{\text{packet}})$ | $\sim 500\,\text{B/s}$ | $\sim 500\,\text{B/s}$ |

**Key observation:** NWP's steady-state bandwidth is independent of $n$ because gossip replaces maintenance traffic. Federated averaging's bandwidth is dominated by model size, not node count — it does not benefit from efficient discovery.

#### Memory Per Node

| Baseline | Memory | At $n=50$ | At $n=10^6$ |
|----------|--------|-----------|-------------|
| **NWP (this work)** | $O(K \log n)$ | **3.6 KB** | **32 KB** |
| Random routing | $O(n)$ | 4 KB | 80 MB |
| Centralized coordinator | $O(1)$ client, $O(n)$ server | 0.1 KB / 4 KB | 0.1 KB / 80 MB |
| Standard gossip | $O(n)$ | 4 KB | 80 MB |
| Federated averaging | $O(1)$ client, $O(n)$ server | 0.1 KB / 4 KB | 0.1 KB / 80 MB |
| Static graph | $O(d)$ | $\approx 0.3$ KB (d=4) | 0.3 KB |

NWP is the only decentralized protocol with sublinear memory. Random routing and standard gossip require $\Theta(n)$ entries because they lack a hierarchical index. The Kademlia k-bucket structure (§4.2) compresses the routing table to $O(K \log n)$ without sacrificing global reachability.

*Note: federated averaging and centralized coordinator have asymmetric memory: the server must store $O(n)$ client metadata, which becomes prohibitive above $n \approx 10^5$ on commodity hardware.*

#### Fault Tolerance

| Baseline | SPOF? | Node failure recovery | Network partition recovery |
|----------|-------|----------------------|---------------------------|
| **NWP (this work)** | **No** | Automatic (Apoptosis eviction, gossip re-discovery) | Automatic (bootstrap cycle, peer cache) |
| Random routing | No | Gradual (lost contacts degrade routing) | Gradual (random probes eventually find survivors) |
| Centralized coordinator | **Yes** — coordinator failure = total loss | Server failure kills all routing; requires manual restart | No recovery during partition |
| Standard gossip | No | Gradual (fanout covers losses) | Gradual (epidemic heals after reconnect) |
| Federated averaging | **Yes** — aggregator is SPOF | Aggregator failure = training halt; state recovery required | No gradient flow during partition |
| Static graph | **Yes** — every edge is a SPOF for those nodes | No recovery; edge failure disconnects paired nodes | Permanent partition on cut |

NWP is the only architecture with **no single point of failure**, **automatic eviction** of dead nodes, and **self-healing** recovery after network partitions. The Apoptosis system (§6.4) provides bounded staleness guarantees: dead entries are evicted within at most $T_{\text{stale}} + \text{PING\_TIMEOUT} = 310$ seconds.

#### Scalability Ceiling

The maximum practical network size before the architecture requires re-engineering or fails:

| Baseline | Max $n$ | Bottleneck |
|----------|---------|------------|
| **NWP (full mesh regime)** | $\sim 100$ | Single-thread UDP socket drain: $n^2$ PINGs exceed $10^4$ packets/tick |
| **NWP (iterative routing regime)** | $> 10^6$ | Theoretical: Kademlia FIND_NODE with $O(\log n)$ hops |
| Random routing | $\sim 10^4$ | Memory: $O(n)$ routing table exceeds RAM at $n > 10^5$ |
| Centralized coordinator | $\sim 10^3$ | Server bandwidth: $O(n^2)$ broadcast at scale |
| Standard gossip | $\sim 10^4$ | Memory: $O(n)$ routing table + bandwidth: $O(b \cdot n)$ per round |
| Federated averaging | $\sim 10^5$ | Straggler effect: slowest node determines round time |
| Static graph | $\sim 10^6$ | No discovery overhead, but failure tolerance is zero |

NWP supports two operating regimes: **full mesh** for small-to-medium clusters ($n \leq 100$) with $\Theta(n^2)$ convergence messages, and **iterative Kademlia routing** for large-scale deployment ($n > 100$) with $O(\log n)$ lookup hops. The Kademlia FIND_NODE/FIND_VALUE mechanism (§2.3) is already specified in the wire protocol; adding iterative multi-hop lookup requires only engine-loop changes, not protocol changes.

#### Learning Efficiency

For neural computation subsystems, compare learning signal quality across architectures:

| Baseline | Learning signal | Gradient staleness | Communication per update |
|----------|----------------|-------------------|-------------------------|
| **NWP Hebbian STDP** | Local: $\Delta w = \eta(a_{\text{pre}}a_{\text{post}} - \lambda w)$ | Zero (local) + gossip | $O(K_{\text{gossip}} \cdot s_{\text{synapse}})$ per tick |
| Federated averaging | Global: $\theta_{t+1} = \sum \frac{n_k}{n} \theta_k$ | One round delay | $O(\text{model})$ per round |
| Standard gossip (learning) | Distributed: exchange weights with peers | $O(\log n)$ rounds to converge | $O(b \cdot \text{model})$ per round |

NWP's Hebbian learning is fundamentally different: it is **fully local** (no global aggregation), which eliminates gradient staleness and straggler effects. The trade-off is that Hebbian plasticity learns distributed representations rather than optimizing a global objective. For applications where local predictive coding suffices (sensor networks, distributed control, continual learning), NWP avoids the bandwidth and synchronization costs of federated approaches entirely.

### 10.3 Aggregate Comparison Matrix

| Criterion | NWP | Random | Centralized | Gossip | FedAvg | Static |
|-----------|-----|--------|-------------|--------|--------|--------|
| Convergence speed | ★★★★ | ★★ | ★★★★★ | ★★★★ | — | — |
| Message efficiency | ★★★ | ★★ | ★★★★★ | ★★★★ | ★★★ | ★★★★★ |
| Steady-state bandwidth | ★★★★★ | ★★★ | ★★★★★ | ★★★ | ★★ | ★★★★★ |
| Memory scaling | ★★★★★ | ★★ | ★★★ | ★★ | ★★★ | ★★★★★ |
| Fault tolerance | ★★★★★ | ★★★ | ★ | ★★★ | ★ | ★ |
| No SPOF | ✓ | ✓ | ✗ | ✓ | ✗ | ✓ |
| Self-healing | ✓ | ✗ | ✗ | ✓ | ✗ | ✗ |
| Sublinear memory | ✓ | ✗ | ✗ (server) | ✗ | ✗ (server) | ✓ |
| Straggler immunity | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ |
| Scalability ($>10^3$) | ✓ | ✗ | ✗ | ✗ | ✓ (comm.) | ✓ |

### 10.4 When Each Baseline Wins

No single architecture dominates all regimes. The following table identifies conditions under which each baseline would be preferred over NWP:

| Baseline | Wins when | Why |
|----------|-----------|-----|
| Centralized coordinator | $n < 20$, trusted environment, operator can tolerate SPOF | Lower convergence latency ($O(1)$ rounds vs. $O(\text{RTT})$) and message count ($O(n)$ vs. $\Theta(n^2)$) |
| Standard gossip | $n \sim 10^2$, rapid convergence needed, no sublinear memory constraint | Gossip converges in $O(\log n)$ rounds with $O(n \log n)$ messages vs. NWP's $\Theta(n^2)$ in full-mesh regime |
| Federated averaging | Global optimization objective required, model is small, nodes are reliable | FedAvg provably minimizes a global loss function; Hebbian STDP optimizes local prediction error, not global loss |
| Static graph | Zero-discovery overhead critical, topology known a priori, failures are tolerable | No bootstrap, no maintenance messages; $O(1)$ per-node memory |
| Random routing | Extreme simplicity, no protocol state at all | No k-buckets, no DHT, no ACK tracking — works with any message-passing substrate |

### 10.5 Summary

Across the six architectures, NWP occupies a unique position in the design space:

- **Only architecture with** simultaneously: sublinear memory ($O(K \log n)$), no single point of failure, automatic self-healing, and straggler immunity.
- **Convergence is RTT-bound** for $n \leq 35$, matching centralized approaches within the regime where decentralization matters most.
- **Message count is $\Theta(n^2)$** during bootstrap (full-mesh regime) — the primary weakness vs. gossip or centralized approaches. This is a deliberate trade-off: full mesh eliminates lookup latency at the cost of $O(n^2)$ messages. The iterative Kademlia path (§8.2) exists as a future scaling option.
- **Steady-state bandwidth is $O(1)$**, independent of $n$, matching static graphs and centralized architectures while exceeding epidemic and federated approaches.
- **Fault tolerance is the strongest** of any baseline: NWP is the only architecture with triple redundancy (no SPOF + automatic eviction + self-healing after partition).

---

## 11. Formal Protocol Specification

### 11.1 BNF Grammar

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

### 11.2 Constants

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

### 11.3 State Machine Summary

```mermaid
stateDiagram-v2
    state "NODE LIFECYCLE" as NODE
    [*] --> INIT
    INIT --> BOOTSTRAP : bind + channels
    BOOTSTRAP --> ACTIVE : seeds contacted
    BOOTSTRAP --> IDLE : no seeds found
    ACTIVE --> SHUTDOWN : shutdown signal
    ACTIVE --> RECOVERY : partition detected
    RECOVERY --> ACTIVE : reconnected
    IDLE --> BOOTSTRAP : incoming PING

    state "PACKET LIFECYCLE (reliable)" as PACKET
    [*] --> CREATED
    CREATED --> QUEUED
    QUEUED --> SENT : UDP send
    SENT --> WAITING_ACK
    WAITING_ACK --> DONE : ACK received
    WAITING_ACK --> RETRANSMIT : timeout
    RETRANSMIT --> WAITING_ACK : retries < N
    RETRANSMIT --> DROPPED : retries >= N
    WAITING_ACK --> DROPPED : weight < threshold
    DONE --> [*]
    DROPPED --> [*]

    state "DHT ENTRY LIFECYCLE" as DHT_ENTRY
    [*] --> DISCOVERED : PING/PONG/NODES
    DISCOVERED --> INSERTED : upsert into bucket
    INSERTED --> STALE : last_seen > 300s
    STALE --> PING_SENT : periodic maintenance
    PING_SENT --> DISCOVERED : PONG received (re-insert)
    PING_SENT --> DEAD : no PONG within 10s
    INSERTED --> DEAD : fail_count >= 3
    INSERTED --> DEAD : latency > 500ms
    DEAD --> EVICTED : apoptosis sweep
    EVICTED --> [*]

    state "ENGINE TICK LIFECYCLE" as TICK
    [*] --> RECV : Phase 1
    RECV --> SEND : Phase 2
    SEND --> COMPUTE : Phase 3
    COMPUTE --> RETRANSMIT_CYCLE : Phase 4
    RETRANSMIT_CYCLE --> CLEANUP : Phase 5
    CLEANUP --> YIELD_CYCLE : Phase 6
    YIELD_CYCLE --> RECV : next tick
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
6. Cormen, T. H., Leiserson, C. E., Rivest, R. L., & Stein, C. (2022).
   *Introduction to Algorithms* (4th ed.). MIT Press. — Complexity bounds
   for distributed hash tables and amortized analysis.
