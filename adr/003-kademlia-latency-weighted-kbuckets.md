# ADR 3: Kademlia DHT with Latency-Weighted k-Buckets

**Status:** Accepted  
**Date:** 2026-04 (initial), reaffirmed 2026-06  
**Deciders:** Project author

---

## Context

A decentralized P2P runtime requires a mechanism for nodes to discover each other without a central registry. The options:

- **Centralized registry** (e.g., etcd, Consul) — simplest but introduces a single point of failure
- **Gossip-based discovery** (e.g., SWIM, Serf) — eventual convergence, no guaranteed lookup bounds
- **Distributed Hash Table** (e.g., Kademlia, Chord, Pastry) — provable O(log N) lookup bounds, well-studied
- **mDNS / LAN broadcast** — limited to local networks
- **Blockchain registry** — decentralized but orders of magnitude too slow

The runtime needs deterministic lookup behavior, known complexity bounds, and a well-studied failure model. Kademlia is the most battle-tested DHT in production (BitTorrent, IPFS, Ethereum).

Standard Kademlia uses XOR distance for bucket placement and least-recently-seen (LRU) eviction. This maximizes reachability but ignores network quality — a peer that responds but is 500ms away is treated the same as a peer 5ms away.

## Decision

Use **Kademlia-style XOR-distance routing** with the following modifications:

1. **XOR for bucket placement** (unchanged from standard Kademlia) — guarantees global reachability
2. **Latency-weighted ranking and eviction** within each bucket — closest (fastest) peers ranked first
3. **K=20** bucket capacity (increased from standard K=8 — BitTorrent uses K=8, but the lower connectivity of learning workloads benefits from larger routing tables)
4. **256 buckets** (160-bit address space extended to 256 bits for compatibility with cryptographic hashes; only the first 160 bits are used for routing)
5. **Latency EMA smoothing**: `latency = latency × 0.7 + sample × 0.3` — prevents latency spikes from causing immediate eviction
6. **Failure tracking**: after 3 consecutive failures, peer is evicted
7. **Node types**: General, Language, Reasoning, Memory, Vision, Audio, Consensus, Gateway — enables cluster-aware gossip (same-type peers prioritized during Hebbian exchange)

Insertion algorithm:
```
fn insert(bucket, peer):
    if bucket has room (< K):
        add peer, sort by latency
    else if peer.latency < bucket.worst().latency:
        evict worst peer, add new peer, sort
    else:
        discard peer (not a quality improvement)
```

## Consequences

**Positive:**
- O(log N) routing complexity with provable bounds
- Latency-weighted eviction naturally biases routing tables toward fast, reliable peers
- EMA smoothing prevents flapping from transient latency spikes
- Node type labels enable content-aware gossip without breaking the DHT structure
- Deterministic behavior with fixed seeds (enables ADR-7)

**Negative:**
- 256 buckets × K=20 = max 5,120 peer entries per node, ~100KB RAM (acceptable but non-trivial)
- Latency-weighted eviction could theoretically create routing holes if a slow peer is the only node in a bucket (unlikely at scale, possible at low node counts)
- Extended 256-bit address space is not standard — compatibility with other Kademlia implementations would require a 160-bit shim

**Empirical results:**
- DHT converges in ≈3.0s across all tested configurations (10, 25, 50 nodes)
- Latency-weighted eviction tested via adversary simulation: under high-latency injection, the routing table self-heals within ≈5s of latency stabilization
- Benchmark data: `results/routing*.csv`

## Related

- ADR-1: Engine loop — DHT maintenance runs during Phase 5 (apoptosis sweep)
- ADR-2: UDP transport — DHT PING/PONG messages use BestEffort reliability tier
- ADR-6: Sparse gossip — gossip targets are selected from the DHT routing table, cluster-biased by node type

## References

- Maymounkov & Mazières (2002). Kademlia: A peer-to-peer information system based on the XOR metric. *IPTPS*.
- BitTorrent Kademlia (DHT) implementation — K=8, 160-bit address space
- IPFS DHT (Kademlia + S/Kademlia modifications) — production-scale reference
- Crosby, S., & Wallach, D. (2003). An analysis of BitTorrent's DHT. *Rice University Technical Report*.
