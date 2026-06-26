# Design Decisions: Building a P2P Neural Runtime from Scratch

*June 2026*

---

## Why Another DHT?

The internet has no shortage of distributed hash tables. Kademlia (2002) is the canonical design — used by BitTorrent, Ethereum, IPFS. Chord, Pastry, and CAN each carved out their niche. So why build another one?

The answer is in the question's premise: every existing DHT was designed for **data storage and retrieval**. NWP is designed for **distributed neural computation** — a fundamentally different workload with different constraints:

| Constraint | Storage DHT | Neural DHT |
|-----------|------------|------------|
| **Traffic pattern** | Lookup-driven, bursty | Periodic, uniform PING/PONG |
| **Topology requirement** | Find any one node with the data | Every node knows every peer |
| **Latency tolerance** | Sub-second lookups | 1s convergence is acceptable |
| **Peer churn handling** | Graceful, eventual | Must converge deterministically |
| **Node identity** | Content-addressed (hash of data) | Random 256-bit substrate IDs |
| **Steady state** | Idle between lookups | Periodic maintenance forever |
| **Verification** | Probabilistic | Deterministic (reproducible) |

The storage-DHT assumption we had to unlearn: **that full mesh is wasteful**. For a neural substrate where every node's activations can influence every other node's next state, a complete peer graph isn't a luxury — it's the baseline.

## Decision 1: Epidemic Broadcast Over Iterative Routing

### The Tension

Standard Kademlia is elegant: XOR distance metric → binary tree over ID space → $O(\log n)$ lookup hops. A lookup visits $H \approx \log_{K/2} n$ nodes, each returning $K$ nearer candidates.

For $n=50$, that's $H \approx \log_{10} 50 \approx 2$ hops. But those hops are **sequential** — each requires a round-trip to a new node before the next can proceed.

### The Alternative

Instead of iterative routing, we flood the network with PING messages at startup. Every node PINGs every known peer. Peers respond with PONG. The result: after $n(n-1)$ messages, every node knows every peer.

Is this wasteful? Yes — $O(n^2)$ messages versus $O(n \log n)$ for iterative routing. **But the wall-clock time is better.** At $n=50$, the critical path is:

- **Iterative routing:** $H$ sequential RTTs ≈ 2 × 30ms ≈ 60ms (plus per-hop processing)
- **Epidemic flood:** 1 round-trip (PING → PONG) ≈ 30ms

The trade-off became clear: message complexity costs bandwidth, not time. On a localhost simulation with a shared UDP socket, bandwidth is cheap. The wall-clock dominance of a single RTT is the binding constraint.

**Key insight:** When $n^2 < \nu \cdot \text{RTT}$ (where $\nu$ is socket throughput), epidemic broadcast completes in the same wall time as a single iterative hop. For $n=50$ on localhost, $n^2 = 2500$ messages at ~100μs each ≈ 250ms, plus one RTT ≈ 30ms — total ~280ms. Iterative routing would need at least 2 sequential RTTs ≈ 60ms plus processing, but crucially the sequential nature means no parallelism. The flood wins because **all PINGs are parallel**.

We made this decision explicit in the architecture: for $n \leq 100$, epidemic broadcast. Beyond that, we preserve the Kademlia FIND_NODE mechanism as a fallback. The code has both paths; the threshold is a configurable constant.

## Decision 2: Single-Threaded Event Engine

### The Conventional Wisdom

Network runtimes use async I/O (tokio, async-std) or thread pools (Rayon, crossbeam). The argument is throughput: while one thread blocks on I/O, another computes.

### Why We Rejected It

Every networked system I've built before had one thing in common: async debugging is a nightmare. Stack traces are useless. Backpressure is non-obvious. The "colored function" problem (sync functions can't call async ones) infects the entire codebase.

NWP's engine loop runs on **exactly one OS thread**. The structure:

```
loop {
    1. recv_from() UDP (non-blocking, 1ms timeout)
    2. Process received packets
    3. Send outbound packets
    4. Neural computation (forward pass + Hebbian)
    5. Retransmit scan
    6. Apoptosis sweep (every 1000 ticks)
}
```

This is essentially a game loop. Games have been doing this for decades and pushing millions of frames per second. The constraints are the same: predictable timing, bounded work per frame, no hidden latency.

### The Measurement

At $n=50$, each tick processes ~100 packets (recv + send), updates routing table entries, and runs maintenance. Total tick time: **~60μs**. The tick budget is 1ms. That's 94% idle. A single thread is nowhere close to saturation.

The real win: **deterministic debugging**. I can pause a thread at any tick boundary, inspect state, and reason about causality. Try that with a tokio task graph spanning 50 UDP sockets.

## Decision 3: Zero-Copy FlatBuffer Wire Format

### The Problem

JSON, BSON, even CBOR: they all parse. They allocate. They copy. For a protocol where every PING/PONG triggers a deserialization on every node, parsing overhead accumulates.

Standard messagepack deserialization of a 48-byte PING body involves:
1. Decode varint lengths (branching)
2. Allocate Vec<u8> for strings
3. Copy bytes from buffer to allocation
4. Build struct with checked field access

Call that ~500ns on a modern CPU. At $n=50$ with each node receiving ~50 packets per tick at 1000 ticks/s, that's 50 × 1000 × 500ns = 25ms per second in parsing overhead — 2.5% of one core.

### The Alternative

FlatBuffer zero-copy: the body layout is defined by byte offsets. Reading `sender_id` means loading `body[0..32]` as a `[u8; 32]`. No allocation, no branching, no copy. The cost is exactly one cache-line fill.

```rust
// Zero-copy: one bounds check, then return a slice
pub fn sender_id(&self) -> &[u8; 32] {
    &self.body[..32].try_into().unwrap()
}
```

The trade-off: **brittle format**. If the sender_id offset changes, every node breaks simultaneously and silently. There's no schema evolution, no forward compatibility. Every protocol change requires a version bump.

We accepted this because the protocol is **not internet-scale**. It's a research runtime where every node runs the same binary. Schema evolution is a compile-time event, not a network-level concern.

## Decision 4: K=20 k-buckets with Latency Sorting

### Canonical Kademlia

Kademlia buckets by XOR distance. Each bucket stores $K$ nodes. When a bucket is full and a new node is discovered, the **least-recently seen** entry is pinged. If it responds, the new node is rejected. If it doesn't, the new node replaces it.

### Our Modification

We keep $K=20$ and bucket by XOR prefix. But insertion sorts by **latency** (EMA-smoothed), not recency:

```
insert(entry):
    bucket = routing_table[id]
    if bucket.len() < K:
        append, sort_by_latency, return ACCEPTED
    if entry.latency_ms < worst_in_bucket:
        pop_last(), append(entry), sort_by_latency, return ACCEPTED
    return REJECTED
```

**Why?** Recency-based eviction assumes churn is uniform. In a neural substrate, churn isn't uniform — it's correlated. When a cluster of nodes goes down (power failure, network partition), they all stop responding simultaneously. LRU would evict them one by one over $K$ insertion attempts, keeping stale entries alive for seconds.

Latency-based eviction is **anti-fragile by design**: a slow node is more likely to be a problem than a recently-contacted node. The exponential moving average (70% old, 30% new) prevents latency spikes from causing flapping.

**The cost:** $O(K \log K)$ per insertion vs $O(1)$ for standard Kademlia's LRU. At $K=20$, $\log K \approx 4$ — cost is negligible.

## Decision 5: Apoptosis as a Memory Management Primitive

Apoptosis (programmed cell death) is usually a biological metaphor in ML papers. In NWP, it's a concrete system: a Phase 5 sweep that evicts stale routing entries, expires pending PINGs, and purges orphaned reliable frames.

The design decision was **not** whether to do cleanup — every protocol needs it — but whether to make it a centralized, synchronous sweep or a distributed, asynchronous process.

We chose synchronous sweep because:
1. **Deterministic death counts:** every run with the same inputs produces the same deaths at the same ticks. Asynchronous cleanup would be non-deterministic.
2. **Single point of causality:** the apoptosis report goes to stderr immediately after the sweep. No wondering "did that entry get evicted yet?"
3. **Death spiral detection:** the sweep can compute aggregate statistics and detect cascading failures in $O(|R| + |P|)$ time.

The downside: **worst-case latency spike**. If all 5120 routing entries are being evicted simultaneously, the sweep takes ~100μs. We accept this because it happens at most once per second and the tick budget absorbs it.

---

*Next: [Failed Experiments — what broke and what we learned](02-failed-experiments.md)*
