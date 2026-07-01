# Tutorial 04: DHT Routing Deep Dive

**Goal:** Understand the modified Kademlia DHT — XOR distance, latency-weighted
k-buckets, the four-step bootstrap sequence, and how to visualise the routing
table in real time.

**Estimated time:** 25 minutes

---

## 1. Standard Kademlia vs NWP's Hybrid

Standard Kademlia (used by BitTorrent, IPFS, Ethereum) places nodes into
k-buckets based solely on **XOR distance** and evicts the **least recently
seen** entry when a bucket is full.

NWP's hybrid changes the eviction policy:

| Property | Standard Kademlia | NWP |
|----------|------------------|-----|
| Bucket placement | XOR distance | XOR distance (same) |
| Bucket size (K) | 20 | 20 (same) |
| Eviction policy | Least recently seen | **Highest latency** |
| Latency tracking | None | EMA-smoothed RTT (α=0.3) |
| Failure tracking | Timeout-based | Consecutive fail count (≥3 = eviction) |
| Node types | Not used | 8 types: General, Language, Reasoning, etc. |

**Why the change?** In a neural computation network, you want the fastest
connections for gradient exchange. A high-latency peer slows down Hebbian
gossip and increases the chance of stale gradients. By evicting the slowest
entry first, NWP naturally optimises for low-latency communication while
preserving Kademlia's XOR-distance reachability guarantee.

## 2. The Routing Table

The routing table is defined in `src/dht.rs`:

```rust
pub struct RoutingTable {
    pub buckets: [KBucket; 256],   // Indexed by XOR prefix length
    pub local_id: NodeId,          // 256-bit random ID
    pub local_addr: SocketAddr,
    pub local_type: NodeType,
}

pub struct KBucket {
    pub entries: Vec<NodeEntry>,   // Sorted by latency (fastest first)
    pub max_size: usize,           // K = 20
}
```

**256 buckets** correspond to the 256 bits of a NodeId. Bucket `i` contains
nodes whose XOR distance shares the first `i` bits with the local node and
differs at bit `i+1`.

### Bucket Index Calculation

```rust
fn bucket_index(local: &NodeId, other: &NodeId) -> Option<u8> {
    let xor = local.0.iter()
        .zip(other.0.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<_>>();
    for (i, &byte) in xor.iter().enumerate() {
        if byte != 0 {
            let msb = 7 - byte.leading_zeros() as u8;
            return Some((31 - i as u8) * 8 + msb);
        }
    }
    None // same node
}
```

## 3. Entry Insertion and Eviction (in Pseudocode)

```rust
fn upsert(&mut self, entry: NodeEntry) -> Result {
    let bucket = self.bucket_mut(&entry.id)?;

    // Already known — update latency, reset failure count
    if let Some(existing) = bucket.find_mut(&entry.id) {
        existing.latency_ms = 0.7 * existing.latency_ms + 0.3 * entry.latency_ms;
        existing.fail_count = 0;
        existing.last_seen = Instant::now();
        bucket.sort_by_latency();
        return Ok(Accepted);
    }

    // Room in bucket — append and sort
    if bucket.entries.len() < K {
        bucket.entries.push(entry);
        bucket.sort_by_latency();
        return Ok(Accepted);
    }

    // Bucket full — evict worst if new peer is faster
    let worst = bucket.entries.last().unwrap();
    if entry.latency_ms < worst.latency_ms {
        bucket.entries.pop();
        bucket.entries.push(entry);
        bucket.sort_by_latency();
        return Ok(Accepted);
    }

    Err(Rejected) // New peer is slower than worst — reject
}
```

**Intuition:** XOR distance distributes nodes across buckets. Within each
bucket, latency determines survival. A slow node cannot fill a bucket
(anti-pollution). A fast node always finds a home.

## 4. The Bootstrap Sequence

When a node starts, it must find peers. The bootstrap follows a four-step
cascade (from `src/dht.rs`):

```rust
pub fn bootstrap(&mut self) {
    // Step 1: Peer cache file
    if let Some(peers) = self.load_peers_cache() {
        for peer in peers {
            self.send_ping(peer);
        }
        return; // Found cached peers — don't need seeds
    }

    // Step 2: DNS seeds (SRV record)
    if let Some(seeds) = self.resolve_dns_seeds("_dht.seeds.neuron-wire.io") {
        for seed in seeds {
            self.send_ping(seed);
        }
        return;
    }

    // Step 3: Hardcoded seed addresses
    for addr in SEED_NODES {
        self.send_ping(*addr);
    }
    if !SEED_NODES.is_empty() {
        return;
    }

    // Step 4: Passive — wait for incoming PINGs
    eprintln!("[DHT] No seeds — listening passively");
}
```

In simulation mode, the simulator **injects known peers** directly into each
node's routing table before launch, bypassing the bootstrap sequence:

```rust
// src/simulator.rs — during node creation
for engine in &mut engines {
    for other in &node_info {
        if engine.addr() != other.addr {
            engine.dht_mut().inject_peer(other.into());
        }
    }
}
```

This is why the simulator achieves convergence in ~3 seconds — nodes start with
all peers already known and just need to verify them via PING/PONG.

## 5. Message Handling State Machine

When a node receives an NWP frame, the DHT handler processes it as follows:

```
Ingress Frame
    │
    ├─ msg_type == 7 (PING)
    │   → Parse sender NodeId from body[0..32]
    │   → Upsert into routing table
    │   → Extract ping_seq from body[44..48]
    │   → Send PONG with same ping_seq
    │
    ├─ msg_type == 8 (PONG)
    │   → Parse sender NodeId + ping_seq
    │   → Look up pending PING by ping_seq
    │   → Calculate RTT = now - sent_time
    │   → EMA update: latency = 0.7×old + 0.3×RTT
    │   → Upsert into routing table
    │
    ├─ msg_type == 9 (FIND_NODE)
    │   → Parse target_id from body[0..32]
    │   → Get K nearest nodes from routing table
    │   → Send NODES response with target_id + entries
    │
    ├─ msg_type == 10 (NODES)
    │   → Parse target_id + entry list
    │   → For each entry: upsert into routing table
    │   → If not yet converged, PING each new entry
    │
    └─ otherwise → ignore / forward to event subscribers
```

## 6. Visualise the Routing Table

You can inspect the routing table at runtime using the simulator's CSV output:

```bash
# Run with output
cargo run --release --example simulate -- --nodes 10 --duration 30 --output-dir results/routing-debug

# Inspect per-node peer progression
column -t -s, results/routing-debug/routing.csv | head -30
```

For a deeper view, the `raw/` directory contains per-node JSONL event logs
(when not in paper mode):

```bash
# Run without paper mode to get verbose logs
cargo run --release --example simulate -- --nodes 5 --duration 20 --output-dir results/verbose

# Inspect node 0's event log
cat results/verbose/raw/node_000.jsonl | head -20
```

Each line in the JSONL is a structured event:

```json
{"tick":150,"event":"dht_pong","peer":"127.0.0.1:9002","rtt_ms":0.12,"latency_ema":0.15}
{"tick":200,"event":"dht_insert","peer":"127.0.0.1:9002","bucket":42,"latency_ms":0.15}
{"tick":250,"event":"dht_converged","peers_known":9,"total_peers":9}
```

## 7. Node Types and Cluster-Biased Gossip

Each NWP node has a type, stored in the `NodeEntry`:

```rust
pub enum NodeType {
    General = 0,
    Language = 1,
    Reasoning = 2,
    Memory = 3,
    Vision = 4,
    Audio = 5,
    Consensus = 6,
    Gateway = 7,
}
```

The Hebbian gossip system uses node types to bias peer selection during
gradient exchange — same-type peers are prioritised. This enables
**cluster-aware learning**: a Language node preferentially exchanges gradients
with other Language nodes, while still maintaining general connectivity via
the DHT.

## 8. Freshness and SGA (Sparse Gradient Aging)

The DHT includes a `FreshnessConfig` that controls gradient decay parameters:

```rust
pub struct FreshnessConfig {
    pub half_life_ms: u64,       // Default: 100 ms
    pub max_age_ms: u64,         // Default: 1000 ms (10 × half-life)
    pub drop_threshold: f32,     // Default: 0.001
    pub stale_ping_secs: u64,    // How often to refresh bucket entries
}
```

The `stale_ping_secs` parameter (default 10) controls how often a node refreshes
its DHT entries by re-pinging them. A lower value means more churn detection
but higher bandwidth.

## 9. Try It Yourself

### Tune the Stale Ping Interval

```bash
# Fast refresh (aggressive churn detection)
cargo run --release --example simulate -- --nodes 5 --duration 30 --stale-ping-secs 2

# Slow refresh (conservative bandwidth)
cargo run --release --example simulate -- --nodes 5 --duration 30 --stale-ping-secs 60
```

### Watch Convergence with Different Seed Values

Different seeds produce different NodeId distributions, which affects how
nodes land in buckets:

```bash
for seed in 42 123 456 789; do
    cargo run --release --example simulate -- --nodes 5 --duration 15 --seed $seed \
        --output-dir results/seed-$seed 2>/dev/null
    echo "seed=$seed: $(tail -1 results/seed-$seed/summary.csv)"
done
```

The convergence time should be nearly identical (~3s) regardless of seed —
Kademlia's log(N) scaling property is independent of specific ID distribution.

## Summary

- ✅ NWP uses **XOR distance for bucket placement** (standard Kademlia) but
  **latency for eviction** (the slowest peer in a full bucket gets replaced)
- ✅ **256 k-buckets**, each with K=20 entries, sorted by EMA-smoothed RTT
- ✅ Bootstrap cascade: peer cache → DNS seeds → hardcoded seeds → passive
- ✅ Nodes track **consecutive failures** — evicted after 3 failed pings
- ✅ **Node types** enable cluster-biased gradient gossip
- ✅ In simulation, convergence takes ~3 seconds regardless of network size
  (O(log N) scaling)

In **Tutorial 05**, you'll learn to run reproducible experiments with the
simulator — paper mode, fixed seeds, multi-trial statistics, and CSV export.

## Source Files Referenced

| File | Key Content |
|------|-------------|
| [`src/dht.rs`](../../src/dht.rs) | `RoutingTable`, `KBucket`, `NodeEntry`, `upsert()`, `bootstrap()` |
| [`src/components.rs`](../../src/components.rs) | `EntityId` — 256-bit neuron identifier |
| [`src/types.rs`](../../src/types.rs) | `MsgType` — PING=7, PONG=8, FIND_NODE=9, NODES=10 |
| [`ADRs 003`](../../adr/003-kademlia-latency-weighted-kbuckets.md) | Design rationale for latency-weighted DHT |
| [`ARCHITECTURE.md`](../../ARCHITECTURE.md) | §4.4 — Full DHT routing specification with field offsets |
| [`PROTOCOL_SPEC.md`](../../PROTOCOL_SPEC.md) | §2 — Wire format for every message type |
