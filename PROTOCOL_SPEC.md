# Neuron Wire Protocol Specification v2

> **Status:** Working Draft (matches codebase at commit `896ce7e`)  
> **Version:** 2  
> **Transport:** UDP  
> **Serialization:** Custom fixed-size binary headers + FlatBuffer body

---

## 1. Overview

NWP is a binary wire protocol for decentralized neural computation over UDP. It defines how neural state — activations, gradients, spike events, consensus votes — is serialized, routed, acknowledged, and merged across a dynamic P2P network of heterogeneous nodes.

**Transport identity:** UDP is the production transport for NWP. The crate also
contains legacy/tooling TCP length-prefix helpers in `src/io.rs`, but those
helpers are not the runtime transport and are not used by the production engine.
Operational nodes bind UDP sockets through `src/transport.rs` and
`src/engine_loop.rs`.

A single UDP datagram carries one NWP message:

```
┌─────────────────────────────────────────────┐
│  Transport Header   (16 bytes, mandatory)    │
├─────────────────────────────────────────────┤
│  NWP Message Header (16 bytes, mandatory)    │
├─────────────────────────────────────────────┤
│  NWP Body           (variable, FlatBuffer)   │
└─────────────────────────────────────────────┘
```

Total minimum wire overhead: **32 bytes** per message.

---

## 2. Transport Header

Every UDP datagram begins with a 16-byte transport header. This header handles sequencing, acknowledgment, and staleness — independent of the application-layer message inside.

### 2.1 Wire Format

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 4 | `sequence_number` | `u32` | Monotonic per-sender sequence counter |
| 4 | 4 | `ack_number` | `u32` | Last contiguous sequence number received from this peer |
| 8 | 4 | `ack_bitfield` | `u32` | Bitmask: bit N = received `ack_number + 1 + N` |
| 12 | 4 | `timestamp` | `u32` | Sender's local timestamp in ms (epoch-relative) |

### 2.2 ACK Bitfield Semantics

```
ack_number = 100
ack_bitfield = 0b...1011_0101

Implicitly acknowledged: seq 0..=100
Bit 0 set:     seq 101 received
Bit 1 set:     seq 102 received
Bit 0 clear:   seq 103 NOT received
Bit 2 set:     seq 104 received
...
Bit 31:        seq 132
```

Total coverage: **33 sequence numbers** per ACK (1 × `ack_number` + 32 × bitfield).

### 2.3 Memory Layout (C Struct)

```rust
#[repr(C)]
pub struct TransportHeader {
    pub sequence_number: u32,
    pub ack_number: u32,
    pub ack_bitfield: u32,
    pub timestamp: u32,
}
// Size: 16 bytes
```

---

## 3. NWP Message Header

Immediately following the transport header (offset 16) is a 16-byte NWP message header.

### 3.1 Wire Format

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 4 | `magic` | `[u8; 4]` | Magic bytes: `NWP\0` (`0x4E 0x57 0x50 0x00`) |
| 4 | 1 | `version` | `u8` | Protocol version (currently 2) |
| 5 | 1 | `msg_type` | `u8` | Message type (see §4) |
| 6 | 2 | `flags` | `u16` | Bit flags (see §3.3) |
| 8 | 4 | `body_len` | `u32` | Length of body in bytes (max 1,000,000,000) |
| 12 | 4 | `header_crc` | `u32` | CRC32 of bytes [0..12) |

### 3.2 Memory Layout

```rust
#[repr(C)]
pub struct MessageHeader {
    pub magic: [u8; 4],     // [0x4E, 0x57, 0x50, 0x00]
    pub version: u8,         // 2
    pub msg_type: u8,        // see §4
    pub flags: u16,          // see §3.3
    pub body_len: u32,       // up to 1_000_000_000
    pub header_crc: u32,     // CRC32 of first 12 bytes
}
// Size: 16 bytes
```

### 3.3 Flag Bits

| Bit | Name | Meaning |
|-----|------|---------|
| 0 | `COMPRESSED` | Body is ZSTD/LZ4 compressed |
| 1 | `ENCRYPTED` | Body is encrypted (not yet implemented) |
| 2 | `BROADCAST` | Intended for all peers |
| 3 | `URGENT` | High-priority, process before other messages |
| 4 | `RESPONSE_EXPECTED` | Sender expects a response |

---

## 4. Message Types

Each message type has a fixed-size body layout (except `Data`, which has a fixed header + variable payload).

### 4.1 Ping (type 0)

**Purpose:** DHT keepalive.  
**Fixed body size:** 0 bytes.  
**Reliability tier:** BestEffort.

No body fields. Presence of the message IS the ping.

### 4.2 Pong (type 1)

**Purpose:** DHT keepalive response.  
**Fixed body size:** 0 bytes.  
**Reliability tier:** BestEffort.

No body fields. Echoes the ping sender's timestamp from the transport header.

### 4.3 Command (type 2)

**Purpose:** Control messages — config updates, shutdown, task dispatch.  
**Fixed body size:** 36 bytes.  
**Reliability tier:** BestEffort.

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 4 | `command_id` | `u32` | Unique command identifier |
| 4 | 4 | `prediction_code` | `u32` | Dictionary-encoded prediction type (see §4.3.1) |
| 8 | 4 | `confidence` | `u32` | Raw confidence `[0, 1_000_000]` |
| 12 | 4 | `context_hash` | `u32` | Hash of context for command matching |
| 16 | 4 | `deadline_us` | `u32` | Deadline in microseconds from receipt |
| 20 | 8 | `source_id` | `u64` | Source node/neuron ID |
| 28 | 4 | `target_mask` | `u32` | Brain region mask (see §4.3.2) |
| 32 | 4 | `name_offset` | `u32` | Relative offset to command name string in data region |

Optional variable-length data follows the fixed portion.

#### 4.3.1 Prediction Codes

| Code | Name | Code | Name | Code | Name |
|------|------|------|------|------|------|
| 1 | code | 8 | science | 15 | data |
| 2 | math | 9 | logic | 16 | plan |
| 3 | text | 10 | translate | 17 | tool |
| 4 | vision | 11 | summarize | 18 | multimodal |
| 5 | audio | 12 | qa | 19 | learning |
| 6 | action | 13 | creative | 20 | social |
| 7 | memory | 14 | debug | >20 | unknown |

#### 4.3.2 Brain Region Masks

| Bit | Region | Bit | Region |
|-----|--------|-----|--------|
| 0 | Sensory | 4 | Motor |
| 1 | Language | 5 | Visual |
| 2 | Reasoning | 6 | Audio |
| 3 | Memory | 7 | Executive |

Bit 31 = ALL regions.

### 4.4 Spike (type 3)

**Purpose:** Neural activation event — one neuron fires, affecting connected neurons.  
**Fixed body size:** 32 bytes.  
**Reliability tier:** BestEffort.

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 8 | `source_id` | `u64` | Source neuron ID |
| 8 | 8 | `target_id` | `u64` | Target neuron ID (or 0 for broadcast) |
| 16 | 4 | `spike_type` | `u32` | 0=activation, 1=inhibition, 2=modulation |
| 20 | 4 | `strength` | `u32` | Raw activation strength |
| 24 | 4 | `payload_hash` | `u32` | Hash of optional data payload |
| 28 | 4 | `payload_offset` | `u32` | Relative offset to payload in data region |

### 4.5 Readiness (type 4)

**Purpose:** Signal that a neuron is ready to process a command.  
**Fixed body size:** 20 bytes.  
**Reliability tier:** BestEffort.

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 8 | `neuron_id` | `u64` | Neuron asserting readiness |
| 8 | 4 | `command_id` | `u32` | Command this neuron can handle |
| 12 | 4 | `latency_us` | `u32` | Estimated processing latency (μs) |
| 16 | 4 | `cache_hit` | `u32` | 0 = miss, 1 = hit |

### 4.6 Data (type 5)

**Purpose:** Bulk data transfer — gradient exchange, weight sync, routing tables.  
**Fixed header size:** 24 bytes.  
**Reliability tier:** Data (up to 3 retries).

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 8 | `sender_id` | `u64` | Sender node/neuron ID |
| 8 | 4 | `data_hash` | `u32` | Hash of payload for deduplication |
| 12 | 2 | `content_type` | `u16` | Payload type (see §4.6.1) |
| 14 | 2 | `compression` | `u16` | 0=none, 1=zstd, 2=lz4 |
| 16 | 4 | `original_len` | `u32` | Uncompressed payload length |
| 20 | 4 | `payload_len` | `u32` | Actual payload length |

Payload bytes follow immediately after the 24-byte header.

#### 4.6.1 Content Types

| Code | Type | Code | Type |
|------|------|------|------|
| 0 | Activations | 3 | Weights |
| 1 | Embeddings | 4 | Gradients |
| 2 | Text | 5 | Routing tables |

### 4.7 Consensus (type 6)

**Purpose:** Consensus votes for distributed agreement.  
**Fixed body size:** 24 bytes.  
**Reliability tier:** Consensus (up to 5 retries).

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 8 | `proposal_id` | `u64` | Proposal being voted on |
| 8 | 8 | `voter_id` | `u64` | Voter node/neuron ID |
| 16 | 4 | `confidence` | `u32` | Vote confidence `[0, 1_000_000]` |
| 20 | 4 | `flags` | `u32` | Vote flags |

---

## 5. Node Addressing

### 5.1 Node ID

Nodes are identified by a **256-bit (32-byte) random identifier**:

```rust
pub struct NodeId(pub [u8; 32]);
```

Node IDs are generated randomly at startup. There is no central authority for ID assignment (this is a known limitation — see ADR-8 and FOUNDATIONAL_QNA.md §8).

### 5.2 Neuron ID

Individual neurons within a node are identified by a **64-bit identifier**:

```rust
pub type NeuronId = u64;
```

Enough for ~18 × 10¹⁸ devices.

### 5.3 Node Types

Every node advertises a type label, enabling cluster-aware gossip:

| Type | Use Case |
|------|----------|
| General | Default, no specialization |
| Language | Text processing, NLP |
| Reasoning | Logic, planning |
| Memory | Long-term storage |
| Vision | Image processing |
| Audio | Speech, sound |
| Consensus | Voting coordination |
| Gateway | External interface |

---

## 6. DHT Routing Protocol

The routing layer is a modified Kademlia DHT, called **Latency-Weighted Hybrid Kademlia**.

### 6.1 Distance Metric

XOR distance (standard Kademlia):

```
distance(a, b) = a.node_id XOR b.node_id
```

### 6.2 Bucket Structure

- **256 buckets** (0 = furthest, 255 = nearest)
- **K = 20** entries per bucket
- Bucket index determined by the leading non-zero byte of the XOR distance

### 6.3 Bucket Insertion Policy

```
fn insert(bucket, peer):
    if bucket has room (< K):
        add peer, sort by latency (ascending)
    else if peer.latency < bucket.worst().latency:
        evict worst peer, add new peer, sort
    else:
        discard peer
```

### 6.4 Latency Smoothing

Latency is tracked as an exponential moving average:

```
latency = latency × 0.7 + sample × 0.3
```

This prevents transient latency spikes from causing immediate eviction.

### 6.5 Failure Tracking

After **3 consecutive failures** (timeout or protocol error), the peer is removed from its k-bucket.

### 6.6 Stale Peer Eviction

Peers with `last_seen > 600 s` (10 minutes) are evicted during the apoptosis sweep.

### 6.7 Bootstrap Sequence

Nodes discover the network in priority order:

1. **Peer cache** — saved peer addresses from a previous session (not yet implemented)
2. **DNS seeds** — resolves `_dht.seeds.<domain>` SRV records
3. **Hardcoded seeds** — compile-time seed addresses (user-configured)
4. **Passive listening** — wait for incoming gossip or broadcast messages

Once a node has at least one known peer, it sends a `Ping` to initiate the DHT handshake.

### 6.8 DHT Maintenance

DHT maintenance runs as part of the engine loop's apoptosis phase (~1 s interval):

- Refresh buckets that haven't been touched in 300 s
- Ping up to 1 stale peer per bucket (if any)
- Evict failed peers
- Remove entries exceeding the staleness threshold

---

## 7. Reliability Tiers

NWP defines three reliability tiers, implemented by the `ReliableQueue` in the transport layer:

### 7.1 Tiers

| Tier | Max Retries | Retry Backoff | Used For | Guarantee |
|------|-------------|---------------|----------|-----------|
| BestEffort | 0 | — | Ping, Pong, Command, Spike, Readiness, Gossip | None (fire-and-forget) |
| Data | 3 | 10 ms, 100 ms, 1,000 ms | Gradient DATA frames | At-least-once under low loss |
| Consensus | 5 | Exponential (10 ms, 50 ms, 250 ms, 1 s, 5 s) | Consensus frames | At-least-once under moderate loss |

### 7.2 ACK Protocol

Receivers track the highest contiguous sequence number received from each peer. Every outgoing message carries the current ACK state in its transport header, so acknowledgments are piggybacked on data traffic (no dedicated ACK packets).

### 7.3 Retransmission

Unacknowledged messages are stored in a `ReliableQueue`. On each retransmission tick (~10 ms), the queue is scanned for entries whose `next_retry_at` tick has passed. Expired entries (retries exhausted or gradient weight decayed below threshold) are dropped.

---

## 8. Gradient Weight Decay

Gradients have a tunable weight that decays exponentially with time:

```
weight = e^(-ln(2) × Δt / half_life)
```

- **half_life** = 100 ms (default)
- At Δt = 100 ms: weight = 0.5
- At Δt = 1,000 ms: weight ≈ 0.001 (dropped)

This means a gradient arriving 500 ms late contributes only ~3% of its original value. The system naturally prioritizes fresh information without requiring clock synchronization.

---

## 9. Constants Summary

| Constant | Value | Scope |
|----------|-------|-------|
| Transport header size | 16 bytes | Wire format |
| NWP header size | 16 bytes | Wire format |
| Total wire overhead | 32 bytes | Per message |
| Magic bytes | `NWP\0` (0x4E 57 50 00) | Header verification |
| Protocol version | 2 | Wire format |
| Max body length | 1,000,000,000 bytes | Header validation |
| K (bucket size) | 20 | DHT routing |
| Buckets | 256 | DHT routing |
| Node ID size | 256 bits (32 bytes) | Addressing |
| Neuron ID size | 64 bits (8 bytes) | Addressing |
| ACK window | 33 packets | Transport |
| Latency EMA decay | 0.7 | DHT routing |
| Max failures before eviction | 3 | DHT routing |
| Stale peer timeout | 600 s (10 min) | DHT routing |
| Default gradient half-life | 100 ms | Learning |
| Gradient drop threshold | weight < 0.001 | Transport |
| Learning rate (η) | 0.01 | Hebbian STDP |
| Weight decay (λ) | 0.999 per tick | Hebbian STDP |
| Activation leak | 0.95 per tick | Forward pass |
| Neurogenesis threshold | 0.2 (surprise accumulator) | Neurogenesis |
| Apoptosis sweep interval | 1 s | Engine loop |
| Gossip interval | 500 ticks (~500 ms) | Hebbian |
| Max gossip fanout | 3 | Hebbian |
| Bootstrap methods | 4 (cache, DNS, hardcoded, passive) | DHT |
| Engine loops per second | ~400 KHz–1 MHz | Performance |

---

## 10. Security Considerations

This specification describes the protocol as currently implemented. The following security properties are **not** provided by v2 of the protocol:

- **No authentication** — any node can generate a random NodeId and join the network
- **No encryption** — all wire-format fields are transmitted in cleartext
- **No replay protection** — a captured message can be retransmitted by an attacker
- **No Sybil resistance** — a single attacker can generate unlimited NodeIds
- **No rate limiting** — a misbehaving node can flood the network

These are documented as known limitations. Future protocol versions will address authentication (ed25519 signatures), encryption (Noise Protocol), and Sybil resistance (proof-of-work or stake-based identity).

---

## 11. Wire Format Diagram (Complete Message)

```
Byte 0                                                                      31
     ├─────────────── Transport Header (16) ───────────────┤├─── NWP Header (16) ───┤├─── Body (N) ───┤
     │ seq_num  │ ack_num  │ bitfield │ timestamp │ magic  │ver│typ│flags│body_len│crc32│  ...payload...  │
     0          4          8          12          16       20  21  22   24      28      32              32+N

     All multi-byte fields: little-endian
     Body: FlatBuffer binary (zero-copy, schema-enforced)
```

---

## References

- Maymounkov & Mazières (2002). *Kademlia: A peer-to-peer information system based on the XOR metric.* IPTPS.
- Google FlatBuffers: https://flatbuffers.dev
- ADR-1: Single-Threaded Engine Loop
- ADR-2: UDP Transport with Reliability Tiers
- ADR-3: Kademlia DHT with Latency-Weighted k-Buckets
- ADR-5: FlatBuffer Zero-Copy Serialization
- ADR-6: Sparse Gossip over Full Mesh
- ADR-7: Deterministic Simulation / Paper Mode

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 2 | 2026-06-26 | Initial formal spec written, matching codebase at `896ce7e` |
