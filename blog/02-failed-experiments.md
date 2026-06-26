# Failed Experiments: What Broke and What We Learned

*June 2026*

---

This is not a story of smooth progress. This is a story of bugs that looked like protocol flaws, convergence that refused to happen, and one particularly humbling evening debugging why nodes kept killing each other.

## Experiment 1: Apoptosis — The Node-Killer

### The Bug

Early benchmarks showed abysmal convergence. We'd launch 5 nodes, wait 10 seconds, and get 0/5 converged. The nodes would start, exchange a few PINGs, and then... nothing. Routing entries would appear briefly and vanish.

The culprit was the **apoptosis sweep**, running every 1000 ticks (~1 second):

```rust
// Phase 5: Apoptosis sweep
let report = self.apoptosis.tick(self.tick, &mut self.dht_handler, &mut self.transport);
```

At the time, the sweep evicted any routing entry with `last_seen > 600s`. Seems generous, right? 10 minutes is a long timeout.

The problem was that `last_seen` was initialized to `Instant::now()` at entry creation time, but the DHT bootstrap and routing table insertion happened **before** the PONG response arrived. The sequence was:

```
1. Node A sends PING to Node B
2. Node B adds Node A to its routing table (last_seen = now)
3. Node B sends PONG back
4. ...time passes while convergence proceeds...
5. Apoptosis runs: "has it been 10 minutes?" No, last_seen was 3 seconds ago.
```

Wait, that should be fine. 3 seconds < 600 seconds. So why were entries vanishing?

### The Deeper Issue

The problem wasn't the timeout — it was that entries were being created **before** the bidirectional handshake completed. The routing table `insert()` method accepted any incoming PING and created an entry immediately. If that PING was never answered with a PONG (UDP loss), the entry sat in the routing table with `fail_count = 0` and `last_seen = now`.

The apoptosis sweep checked `fail_count >= 3` as an eviction criterion. But entries created from unidirectional PINGs had `fail_count = 0`, so they wouldn't be evicted by that check either.

So what was evicting them?

### The Real Bug

It was the **k-bucket insertion logic**. When a bucket filled to $K=20$, inserting a new entry required evicting the worst entry. The "worst" was determined by latency. But new entries from PINGs had `latency_ms = 0` (initialized to zero before any RTT measurement). Zero latency is **better** than any measured latency, so new entries would always be accepted and older, validated entries would be evicted.

The fix was **pedestrian**:

```rust
// Before: latency defaulted to 0.0
NodeEntry {
    latency_ms: 0.0,  // ← BUG: appears optimal
    // ...
}

// After: latency starts at a high sentinel value
NodeEntry {
    latency_ms: 10_000.0,  // ← FIX: 10s sentinel, always worse than real
    // ...
}
```

One line. Three days of debugging.

**Lesson:** Default values matter. When a default looks like a good value, it will silently defeat ranking logic. Sentinel values should be obviously wrong (10 seconds for latency, -1 for sequence numbers, `MAX` for anything that competes on minimum).

## Experiment 2: The NWP Frame Offset Miscalculation

### The Bug

The FlatBuffer wire format specifies fixed byte offsets for each field in the body. The PING body layout was:

```
[0..32)  sender_id: [u8; 32]
[32..39) addr: SocketAddr (IPv4: 7 bytes, IPv6: 19 bytes)
[39..40) node_type: u8
[40..44) latency_ms: f32
[44..48) ping_seq: u32
```

But `SocketAddr` varies in size: IPv4 addresses fit in 7 bytes (4 + 2 port bytes + 1 flag), IPv6 in 19 bytes (16 + 2 + 1). The body size calculation used `std::mem::size_of::<SocketAddr>()`, which is **24 bytes** on the Rust standard library (it rounds up to alignment boundaries). The actual serialized size was 7 bytes for IPv4.

The result: every IPv4 PING body was parsed with a 17-byte offset error starting at `node_type`. Latency values read from `body[40..44]` were actually reading the last 4 bytes of the address field plus garbage. PONG responses echoed this garbage back, so RTT calculations produced random values between 10⁻¹⁷ and 10³⁰ seconds.

### Detection

This bug was caught by a **property-based test** that round-tripped 10,000 random IPv4 and IPv6 addresses through the serialization/deserialization path. The test was added after the fifth time I manually checked a PONG body and found `latency_ms = 4.87e27`.

```rust
#[test]
fn test_transport_header_roundtrip() {
    for _ in 0..10_000 {
        let addr: SocketAddr = if rng.gen_bool(0.7) {
            ([rng.gen::<u8>(), rng.gen::<u8>(), rng.gen::<u8>(), rng.gen::<u8>()],
             rng.gen::<u16>()).into()
        } else {
            // IPv6
            (rng.gen::<[u8; 16]>(), rng.gen::<u16>()).into()
        };
        // ...serialize, deserialize, assert_eq
    }
}
```

This test now lives in `src/transport.rs` and runs in CI. It's caught 3 similar alignment bugs since.

**Lesson:** When writing a binary protocol by hand, **do not trust `size_of`**. The compiler's padding rules and the wire format's compact layout are two different things. Write round-trip fuzz tests before you write any application logic. I should have written this test on day zero.

## Experiment 3: The Duplicate Detection That Wasn't

### The Bug

The reliable queue uses sequence numbers and a 32-bit ACK bitfield for loss detection. The invariant: if packet with seq N is acknowledged, all packets with seq < N are implicitly acknowledged (contiguous delivery assumption).

The test for duplicates was:

```rust
if seq <= last_contiguous {
    return; // duplicate
}
```

This works for **in-order** delivery. But UDP is unordered. When packet seq=5 arrived before seq=4, seq=4 was classified as a duplicate because `last_contiguous` was still 3.

The ACK then looked like:

```
ACK bitfield: [1, 1, 1, 1, 0, 1, ...]
```

The sender sees that positions 0-3 and 5 are acknowledged but 4 is missing, so it retransmits seq=4. The receiver gets it and says "I already have this" and drops it — because it was processed out of order, not because it was actually a duplicate.

### The Fix

Change duplicate detection from fully-ordered to sequence-number-based:

```rust
// Before
fn is_duplicate(seq: u32, last_contiguous: u32) -> bool {
    seq <= last_contiguous  // BUG: out-of-order drops valid packets
}

// After
fn is_duplicate(seq: u32, seen: &RoaringBitmap) -> bool {
    !seen.insert(seq)  // false if already present
}
```

RoaringBitmap is compact (1 bit per u32, ~512 KB for all 4 billion sequence numbers). Insertion takes ~10ns. The bitmap persists for the session lifetime.

**Lesson:** "Duplicate" is not the inverse of "in-order". A packet arriving out of order is not a duplicate, and assuming contiguous monotonic ordering on an unordered transport is wrong. This is entry-level networking theory, and I still made the mistake because I was thinking in terms of TCP semantics without writing down the UDP contract explicitly.

## Experiment 4: The 50-Node Convergence Gap

### The Bug

Convergence at 3, 5, 10, and 25 nodes was consistently 3.0 seconds. At 50 nodes, it jumped to 4.0 seconds. The scaling was not continuous.

The cause was **socket saturation**. Each of the 50 nodes sends `n-1 = 49` PINGs at startup. That's `50 × 49 = 2450` PING messages, all arriving at 50 UDP sockets within milliseconds. Each socket's receive buffer filled, and the kernel started dropping packets.

The Linux UDP receive buffer default is 212,992 bytes. Each PING is ~80 bytes with headers. 2450 PINGs × 80 bytes ≈ 196 KB. That fits... barely. But with PONG responses doubling the traffic to ~400 KB in the same window, the buffer overflowed.

### The Failed Fix

My first attempt was to **stagger the PINGs** — have nodes wait a random delay before starting their bootstrap sequence:

```rust
fn bootstrap() {
    sleep(Duration::from_millis(rng.gen_range(0..1000)));  // ← BAD IDEA
    // ...send PINGs
}
```

This made convergence **non-deterministic**. Sometimes it was 3.0 seconds, sometimes 8.0 seconds, depending on the random delays. The worst-case convergence time had no upper bound. I reverted this immediately.

### The Real Fix

The fix was to **accept the saturation** and measure it correctly. The 4.0 seconds at 50 nodes is still $O(1)$ with respect to node count — the saturation adds a constant overhead:

```
Convergence time = RTT + socket_drain_time
```

Where `socket_drain_time ≈ total_messages / (socket_throughput × parallelism_factor)`.

The 1-second gap between 25 nodes (3.0s) and 50 nodes (4.0s) corresponds to the extra time needed to drain 3675 additional messages through a single-threaded recv loop. This is **physics, not a bug**.

We documented this explicitly and moved on. There's no architectural fix for UDP socket bandwidth on a single port — the only solution is to use multiple ports or switch to TCP for bootstrap, which introduces its own problems.

**Lesson:** Not every scaling gap is a bug. Some are fundamental resource constraints. The skill is distinguishing the two. The test was: "Is the gap reproducible with zero variance?" Yes → physics constraint. "Does the gap grow with n?" At 50 nodes, the saturation is constant. At 100 nodes, we'd need to re-evaluate.

## Experiment 5: Zero-Copy PING Processing — The Over-Engineering

### The Failed Attempt

The FlatBuffer zero-copy design was intended to eliminate parsing overhead entirely. But the first implementation tried to map the entire incoming UDP datagram as a `&[u8]` and pass it through the entire engine without copying **anything**.

The problem: the engine runs on a single thread, but the DHT handler and event subscribers run as separate tasks that receive cross-beam channel messages. These tasks **outlive** the UDP receive buffer. So we had to copy the payload into the channel messages anyway.

The 10-line zero-copy path became:

```rust
fn handle_ingress(data: &[u8], src: SocketAddr) {
    let transport_header = TransportHeader::from_bytes(&data[..16]);
    let nwp_frame = &data[16..];
    let body = &nwp_frame[4..]; // strip frame_len
    
    // ...process fields by offset...
    
    // BUT: we still need to send this to events_tx → copy
    let event = IngressEvent {
        transport_header,
        nwp_payload: body.to_vec(),  // ← allocation anyway
        src,
        // ...
    };
    events_tx.send(event);
}
```

The zero-copy was an illusion beyond the first function. The `IngressEvent` owned a `Vec<u8>`, which meant every packet was heap-allocated regardless of our careful offset calculations.

### The Resolution

We kept the FlatBuffer format but accepted the single copy. The benefit is not eliminating allocation — it's eliminating **parsing branches**. Reading `sender_id` is still one bounds check and one slice:

```rust
// Fast path: offset computation, no match on message type
let sender_id = &body[..32];
let addr = &body[32..39];
let node_type = body[39];
let latency_ms = f32::from_le_bytes(body[40..44].try_into().unwrap());
```

This is ~20 CPU instructions versus ~500+ for a structured deserialization. The allocation is one `Vec::with_capacity(128)` per packet, which the allocator satisfies from a hot cache.

**Lesson:** Zero-copy is a direction, not a binary state. Optimize the hot path (field access) even if the cold path (message delivery to subscribers) allocates. The profile showed field access occurring 10× more often than subscriber delivery.

---

*Next: [Lessons Learned — heuristics that survived](03-lessons-learned.md)*
