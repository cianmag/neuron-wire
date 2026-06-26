# ADR 6: Sparse Gossip Over Full Mesh

**Status:** Accepted  
**Date:** 2026-04 (initial), reaffirmed 2026-06 (negative SGA result published)  
**Deciders:** Project author

---

## Context

In a fully connected P2P network, every node could potentially communicate with every other node. Full mesh communication is the simplest approach: each tick, broadcast or multicast to all known peers.

**The problem with full mesh:** Communication cost grows O(N²) — each node sends to every peer. For a 50-node network, full mesh means 2,450 unidirectional connections. For 1,000 nodes, it's ~1M connections. This is infeasible for anything beyond small networks.

Alternatives:
- **Gossip protocols** — each node talks to a small random subset of peers each round. Communication cost O(N log N) or O(N), depending on fanout. Used by SWIM, Cassandra, Redis Cluster.
- **Tree/ring broadcast** — structured topology, O(N) communication per broadcast round, but single point of failure at the root/coordinator.
- **DHT-based routing** — Kademlia itself routes lookups, not periodic state exchange. Learning requires ongoing gradient exchange, not one-shot lookups.

The runtime needs ongoing (per-tick) gradient exchange between learning nodes. Gossip is the natural fit: each node selects a small number of peers to exchange activations with, and information diffuses across the network over multiple rounds.

## Decision

Use **sparse gossip with mutation-weighted peer selection**, where each node communicates with exactly one peer per gossip round (configurable fanout, default 1).

Gossip selection algorithm:
```
fn gossip_target(peers, activation_map) -> Option<PeerId>:
    // 1. Filter to same-type peers (cluster bias)
    same_type = peers.filter(p.type == self.type)
    
    // 2. Weight by mutation rate (prediction error)
    weights = same_type.map(p => p.mutation_weight)
    
    // 3. Select proportional to weight (higher mutation = more gossip)
    selected = weighted_random(same_type, weights)
    
    // 4. Bounded fanout: send to at most 3 peers per round
    return selected
```

Key parameters:
- **Gossip interval:** every 500 ticks (~500ms at 1ms/tick)
- **Max fanout:** 3 peers per gossip round
- **Selection bias:** cluster-biased (same-type first), then mutation-weighted
- **Mutation weight:** computed from prediction error accumulator — "surprised" nodes gossip more

**Sparse Gradient Aging (SGA) experiment:**
A variant of gossip where stale (longer-ungossiped) peers are prioritized. This was the project's main experimental hypothesis: that SGA would reduce bandwidth by focusing pings on stale peers.

The experiment disproved the hypothesis: SGA increased bandwidth 1.9–2.45× across all node counts, and maintenance pings (in either mode) did not measurably improve routing quality once k-buckets were saturated. This negative result is published as `v0.3.0-negative-sga`.

## Consequences

**Positive:**
- O(N) communication scaling — each node sends O(fanout) messages per gossip round, regardless of network size
- Mutation-weighting naturally focuses gossip on nodes that are learning something new
- Cluster bias creates emergent specialization — vision nodes gossip more with vision nodes, etc.
- BestEffort delivery is acceptable — missed gossip rounds are caught on the next round
- The negative SGA result validated a methodological principle: the project runs experiments that can disprove its own hypotheses

**Negative:**
- Information diffusion latency grows with network diameter — worst-case O(diameter × gossip_interval) for a single activation to propagate
- At scale (10³+ nodes), fanout of 3 may be insufficient — adaptive fanout is an open question
- Cluster bias could create disconnected sub-networks if a node type has too few members (mitigated: if no same-type peers, fall back to random selection)
- SGA's bandwidth increase surprised us — it showed that the simpler fixed-interval strategy was more efficient

**Empirical results:**
- SGA bandwidth (50 nodes): 20,325 Kbps vs. fixed: 9,525 Kbps — SGA 2.13× higher
- Routing convergence: identical between modes (3.0s)
- Maintenance pings contributed zero measurable routing quality improvement
- See `results/sga-*` and `results/fixed-*` for raw data

## Related

- ADR-3: Kademlia DHT — gossip targets are selected from the local routing table
- ADR-4: Hebbian learning — gossip exchanges accumulated gradients from STDP updates
- ADR-7: Deterministic simulation — gossip selection is seeded for reproducibility
- Implementation: `src/hebbian.rs` (gossip_target_selection)

## References

- Demers, A., et al. (1987). Epidemic algorithms for replicated database maintenance. *PODC*.
- Van Renesse, R., et al. (2003). Astrolabe: A robust and scalable technology for distributed system monitoring, management, and data mining. *ACM TOCS*.
- Boyd, S., et al. (2006). Randomized gossip algorithms. *IEEE Trans. Info. Theory*.
- Lian, X., et al. (2017). Can decentralized algorithms outperform centralized algorithms? A case study for decentralized parallel stochastic gradient descent. *NIPS*.
