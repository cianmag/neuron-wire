# Architecture Decision Records

> This directory documents the key architectural decisions made in the neuron-wire project.
> Each record explains the context, the decision, and the consequences — so future contributors
> (and the original author) understand why things are the way they are.
>
> Format: [Michael Nygard's ADR template](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)

## Index

| # | Title | Status |
|---|-------|--------|
| 1 | [Single-threaded engine loop (no async runtime)](001-single-threaded-engine-loop.md) | Accepted |
| 2 | [UDP transport with reliability tiers (not TCP)](002-udp-transport-with-reliability-tiers.md) | Accepted |
| 3 | [Kademlia DHT with latency-weighted k-buckets](003-kademlia-latency-weighted-kbuckets.md) | Accepted |
| 4 | [Hebbian STDP learning (not backpropagation)](004-hebbian-stdp-not-backprop.md) | Accepted |
| 5 | [FlatBuffer zero-copy serialization](005-flatbuffer-zero-copy-serialization.md) | Accepted |
| 6 | [Sparse gossip over full mesh](006-sparse-gossip-over-full-mesh.md) | Accepted |
| 7 | [Deterministic simulation / paper mode](007-deterministic-simulation-paper-mode.md) | Accepted |
| 8 | [No persistent storage (in-memory only)](008-no-persistent-storage-in-memory.md) | Accepted |

## Status Meanings

- **Proposed** — under consideration, not yet adopted
- **Accepted** — agreed upon, implementation proceeds
- **Deprecated** — superseded by a later ADR, kept for historical context
- **Superseded** — replaced by a newer ADR
