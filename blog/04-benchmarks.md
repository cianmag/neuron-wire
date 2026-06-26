# Benchmarks: 50 Nodes in 4 Seconds

*June 2026*

---

## Experimental Setup

All benchmarks run on the same hardware with identical configuration:

| Parameter | Value |
|-----------|-------|
| Hardware | Windows 10, Intel Core i7, 16GB RAM, SSD |
| Rust compiler | rustc 1.80+ (LLVM backend) |
| Build profile | `debug` (unoptimized) |
| Command | `cargo run --example simulate -- --paper-mode` |
| Seed | 42 (deterministic) |
| DHT stale ping | 300s (relaxed; no evictions during benchmark) |

The use of `debug` profile is intentional: we benchmark the developer experience, not the optimized one. Release builds would be faster, but debug builds are what contributors run. If the protocol works well unoptimized, it works well everywhere.

## 1. Convergence Scaling

### Raw Data

| Nodes | Trials | Converged | Rate | Convergence Time | Max Peers | Avg Peers | Bandwidth |
|-------|--------|-----------|------|-----------------|-----------|-----------|-----------|
| 3 | 1 | 1 | 100% | 3.00 ± 0.00s | 2/2 | 1.40 | 67 kbps |
| 5 | 10 | 10 | 100% | 3.00 ± 0.00s | 4/4 | 3.52 | 284 kbps |
| 10 | 5 | 5 | 100% | 3.00 ± 0.00s | 9/9 | 8.10 | 1.3 Mbps |
| 25 | 3 | 3 | 100% | 3.00 ± 0.00s | 24/24 | 21.60 | 8.7 Mbps |
| 50 | 3 | 3 | 100% | 4.00 ± 0.00s | 49/49 | 45.35 | 36.5 Mbps |

### Key Observations

**Convergence is $O(1)$ with respect to node count.** The dominant factor is the round-trip time for PING/PONG, not the number of nodes. At 3, 5, 10, and 25 nodes, convergence time is exactly 3.0 seconds — the RTT budget for two PING/PONG cycles with a 1ms tick interval.

At 50 nodes, the time increases to 4.0 seconds. This is not an algorithmic scaling problem — it's **socket saturation**. Each of the 50 nodes sends `n-1 = 49` PINGs at startup, totalling 2450 PING messages. With PONG responses, the total traffic in the first second is ~4900 packets. A single-threaded UDP recv loop processes approximately 2500-3000 packets per second at this message size (~80 bytes). The extra second is the drain time for the socket buffer.

**Zero variance ($\sigma = 0$).** Every trial with seed 42 produces identical results. This is not a coincidence — it's a design constraint. The engine loop is deterministic by construction: seeded RNG, synchronous sweeps, sequence-number monotonicity. A non-deterministic result is treated as a **bug with exit code 3**.

### Convergence Profile (50 nodes)

```
tick=0:      0/2450 connections (0%)
tick=1004:   0/2450 connections (0%)    ← RNG init, nodes starting
tick=2025:   272/2450 connections (11%) ← First PINGs landing
tick=3000:   2220/2450 connections (91%) ← PONG responses arriving
tick=4008:   2450/2450 connections (100%) ← Full mesh achieved
tick=5001+:  stable at 2450/2450       ← No churn
```

The S-curve is characteristic of epidemic flooding: slow start (first PINGs), exponential growth (PONGs trigger more PINGs), then saturation (socket buffer fills, linear drain).

## 2. Steady-State Behaviour

**Apoptosis deaths during steady state: 0.**

Across all benchmarks, no routing entries were evicted, no pending pings expired, no data frames were purged during the convergence + steady-state window. The routing table reaches a fixed point and stays there.

This is the expected result of a DHT with `STALE_PING_S = 300` seconds running a 10-second experiment. But it's still worth measuring: some DHT implementations exhibit churn even in steady state due to unnecessary evictions or ping timeouts. NWP's latency-sorted k-buckets and EMA-smoothed RTT prevent flapping.

### Steady-State Bandwidth (Per Node)

After convergence, each node maintains its routing table with periodic PINGs. The steady-state bandwidth is:

$$B_{\text{steady}} = \frac{n \cdot s_{\text{ping}}}{T_{\text{maintenance}}} \approx \frac{50 \times 80\text{B}}{30\text{s}} \approx 133\text{B/s}$$

In practice, because PINGs are batched with other traffic and the maintenance interval has jitter, measured bandwidth is ~300 B/s per node. This is **independent of $n$** after convergence because maintenance traffic is bounded by the gossip interval, not the routing table size.

## 3. Failure Mode Benchmarks

### Node Death (50% of 10 nodes at t=15s)

| Metric | Value |
|--------|-------|
| Pre-failure convergence | 3.00s |
| Nodes killed | 5 (indices 0-4) |
| Recovery time | 1.00s |
| Min peers post-failure | 4 (of 4 live) |
| Re-converged | Yes |

The recovery time is notable: 1 second vs 3 seconds for initial convergence. The survivors still have each other in their routing tables and don't need to re-discover from scratch. The 1 second is the time needed for the 5 surviving nodes to PING-reconfirm the 4 survivors they already know.

### Partition (50% / 50% of 10 nodes at t=15s)

| Metric | Value |
|--------|-------|
| Pre-failure convergence | 3.00s |
| Partition type | Group A (0-4) / Group B (5-9) |
| Recovery time | 2.00s |
| Min peers post-failure | 4 (of 4 in-group) |
| Re-converged | Yes |

Partition recovery takes 2 seconds — the partition cut was total (no cross-group traffic), so each group must independently re-stabilize, then re-merge when the filter drops. The re-merge is effectively a new bootstrap from each group's perspective.

### Malicious Node (node 0 at t=15s)

| Metric | Value |
|--------|-------|
| Pre-failure convergence | 3.00s |
| Malicious node | Index 0 |
| Recovery time | 0.00s |
| Min peers post-failure | 9 (of 9 live) |
| Re-converged | Yes (never de-converged) |

Malicious node death has zero recovery time because removing a single node from a fully-connected 10-node mesh doesn't break connectivity. The 9 remaining nodes still form a complete subgraph. This is a property of the full-mesh regime: $n-1$ nodes after a single failure are still a clique.

## 4. Adversarial Attack Benchmarks

| Attack | Nodes | Duration | Engine stability | Notes |
|--------|-------|----------|-----------------|-------|
| Bad Packets | 5 | 30s | Survived | 1000+ malformed packets, 0 crashes |
| Corrupted State | 5 | 30s | Survived | 20% churn, convergence re-established |
| Spoofed Identity | 5 | 30s | Survived | Wrong NodeId detected, dropped |
| Replay Attack | 5 | 30s | Survived | Duplicates filtered via RoaringBitmap |
| All | 5 | 120s | Survived | Combined attack, 0 crashes |

The engine survived every adversarial scenario without crashing. The attack vectors validated specific properties:
- **Bad Packets:** parsing resilience — the engine never panics on malformed input
- **Corrupted State:** routing table integrity under churn — stale entries evicted, fresh entries established
- **Spoofed Identity:** identity verification — DHT handler checks UDP source matches claimed NodeId
- **Replay Attack:** idempotency — duplicate sequence numbers dropped by bitmap
- **All:** compound resilience — no cascade failures when multiple attack types run simultaneously

## 5. Baseline Comparisons

Comparative analysis against five standard distributed system architectures. All metrics are analytical or empirically measured at $n=50$.

### Convergence to Full Mesh

| Architecture | Time to full mesh | Derivation | At $n=50$ |
|-------------|-------------------|------------|-----------|
| **NWP (this work)** | $O(\text{RTT} + n^2/\nu)$ | PING/PONG flood with Kademlia routing | **4.0 s** |
| Random routing | $O(n \log n \cdot \text{RTT})$ | Coupon collector | ~55 s |
| Centralized coordinator | $O(1)$ rounds | One registration + one broadcast | ~6 ms |
| Standard gossip | $O(\log n \cdot \text{RTT})$ | Epidemic spread | ~9 ms |
| Static graph | N/A | No discovery | — |

Centralized coordinator wins on speed. It's also a single point of failure. The comparison exposes a fundamental trade-off: **decentralization costs time**.

### Message Complexity (Total Network)

| Architecture | Messages to converge | At $n=50$ |
|-------------|---------------------|-----------|
| **NWP** | $\Theta(n^2)$ PING + PONG | **4,900** |
| Random routing | $O(n^2 \log n)$ | ~9,750 |
| Centralized coordinator | $O(n)$ | 100 |
| Standard gossip | $O(n \log n)$ | ~450 |
| Static graph | 0 | No discovery |

NWP sends more discovery messages because it implements a full-mesh broadcast rather than a spanning tree. The trade-off is **fault tolerance**: NWP has no single point of failure and nodes require zero configuration.

### Steady-State Bandwidth (Per Node)

| Architecture | Bandwidth | At $n=50$ | At $n=10^3$ |
|-------------|-----------|-----------|-------------|
| **NWP** | ~300 B/s | **300 B/s** | **300 B/s** |
| Random routing | $O(k \cdot s_{\text{entry}})$ periodic | ~1 KB/s | ~10 KB/s |
| Centralized coordinator | $O(n)$ heartbeat | ~50 B/s | ~1 KB/s |
| Standard gossip | $O(b \cdot s_{\text{full}})$ per interval | ~10 KB/s | ~10 KB/s |
| Static graph | $O(d \cdot s_{\text{packet}})$ | ~500 B/s | ~500 B/s |

NWP's steady-state bandwidth is the only one that is **constant with respect to $n$**. This is because maintenance traffic is driven by a fixed tick interval, not the routing table size. The gossip interval (30s) bounds the per-node bandwidth regardless of how many peers exist.

### Memory Per Node

| Architecture | Memory | At $n=50$ | At $n=10^6$ |
|-------------|--------|-----------|-------------|
| **NWP** | $O(K \log n)$ | **3.6 KB** | **32 KB** |
| Random routing | $O(n)$ | 4 KB | 80 MB |
| Centralized coordinator | $O(1)$ / $O(n)$ server | 0.1 KB / 4 KB | 0.1 KB / 80 MB |
| Standard gossip | $O(n)$ | 4 KB | 80 MB |
| Static graph | $O(d)$ | ~0.3 KB | 0.3 KB |

NWP is the only decentralized protocol with **sublinear memory**. The Kademlia k-bucket structure compresses the routing table to $O(K \log n)$ without sacrificing global reachability.

## 6. The 50-Node Ceiling

NWP in full-mesh regime is practical up to approximately 100 nodes. Beyond that, the $n^2$ PING flood saturates the single-thread UDP socket:

| $n$ | PINGs | Estimated convergence | Bottleneck |
|-----|-------|----------------------|------------|
| 10 | 90 | 3.0s | RTT |
| 25 | 600 | 3.0s | RTT |
| 50 | 2,450 | 4.0s | Socket drain |
| 100 | 9,900 | ~11s | Socket drain |
| 200 | 39,800 | ~45s | Quadratic growth |

For $n > 100$, NWP preserves the Kademlia FIND_NODE mechanism for iterative multi-hop routing, which scales as $O(\log n)$. The transition is a configurable parameter (`--gossip-interval`), not a code change — both protocols coexist in the wire format.

## 7. Reproducibility Guarantee

Every benchmark result in this document can be reproduced by running:

```bash
cargo run --example simulate -- --nodes 50 --duration 10 --seed 42 --paper-mode
```

The output is deterministic: any machine running any rustc 1.80+ compiler will produce byte-identical CSV files for the same seed. This is ensured by:

1. Seeded RNG for all random operations (JITTER, port allocation, bootstrap delays)
2. Deterministic node startup order (sequential, not parallel)
3. Synchronized simulation start after all nodes are ready
4. No wall-clock-dependent assertions in the measurement path

The seed=42 reference output is checked into the repository under `results/` as a CI-golden file. A PR that changes benchmark output triggers a CI failure.

---

*This is the fourth and final post in the series. The full codebase is at [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire). The ARCHITECTURE.md document contains the complete formal specification.*
