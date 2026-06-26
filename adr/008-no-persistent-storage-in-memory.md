# ADR 8: No Persistent Storage (In-Memory Only)

**Status:** Accepted  
**Date:** 2026-04 (initial)  
**Deciders:** Project author

---

## Context

Most networked services persist state to disk — for crash recovery, data durability, or offline analysis. The standard options include:

- **SQLite** — embedded relational database, zero configuration
- **RocksDB / Sled** — embedded key-value stores optimized for SSDs
- **Append-only logs** — simple, crash-safe, but O(N) read cost
- **Flat files (JSON/CSV)** — simple, human-readable, but no query capability

Each of these adds complexity: database initialization, schema migrations, crash recovery logic, I/O scheduling in the engine loop, and disk space monitoring.

The runtime's state is:
- **Routing table:** ~5,120 peer entries max (256 buckets × K=20), ~100KB
- **Synapse weights:** proportional to neuron count × average fanout — ~100KB–10MB for experimental-scale networks
- **Activation map:** transient per-tick, ~neuron_count × 4 bytes
- **Gossip accumulator:** per-neuron gradient accumulators, cleared after each gossip round

All state is scoped to a single run. There is no cross-run state that must survive a restart.

## Decision

**Keep all runtime state in memory only.** No disk writes during normal operation. No database, no persistence layer, no recovery mechanism.

The simulator writes output to disk (CSV/JSON), but this is an explicit export of experimental results, not runtime state:

```
Runtime:                        Disk:
┌─────────────────────┐         ┌─────────────────────┐
│  Routing Table       │         │  experiment.toml    │
│  Synapse Weights     │  ───►  │  convergence.csv    │
│  Activation Map      │ export │  bandwidth.csv      │
│  Gossip Accumulators │  on    │  routing.csv        │
│  Engine Statistics   │  stop   │  summary.csv        │
└─────────────────────┘         └─────────────────────┘
                                (read on start,
                                 written on stop)
```

The only file read on startup is the experiment configuration (`experiment.toml`). Node state is reconstructed from scratch.

## Consequences

**Positive:**
- No database dependency — keeps the dependency tree at 5 crates (ADR-1)
- No I/O in the engine loop — no disk writes to block on, no fsync calls
- Crash semantics are trivial: if the process dies, state is lost. There's no "recovery" because there's nothing to recover
- Simpler code — no serialization/deserialization of runtime state to disk
- Every run starts from a clean state — no stale state from a previous run can corrupt results
- Reproducibility is strengthened — all state is determined by the config + seed, not by disk history

**Negative:**
- **Node restart = total state loss** — a production deployment cannot recover from crashes without rebuilding the routing table from scratch
- Memory-only semantics limit deployment to runs where state loss is acceptable (research experiments, not production)
- No snapshot/restore for long-running experiments — if a 24-hour experiment crashes at hour 23, data is lost
- Large-scale deployments (>10^4 neurons) may need more memory than available on free-tier VPS (512MB)
- The export-on-stop pattern means mid-experiment analysis requires live-logging to stdout (which simulator supports via JSONL)

**Critical admission:**
This decision makes neuron-wire unsuitable for production deployment today. The runtime cannot survive a process restart, cannot recover from crashes, and cannot serve persistent state to clients. This is acceptable because the project is a research prototype — deployment scenarios are explicitly future work.

The roadmap includes snapshot/restore as part of v0.4.0, once the learning dynamics are better understood and the architecture is stable enough to warrant persistence.

## Related

- ADR-7: Deterministic simulation — CSV export is the boundary between ephemeral runtime state and persistent experimental results
- ARCHITECTURE.md §"Limitations" — item 4 documents the "No persistent storage" limitation
- ROADMAP.md — snapshot/restore listed in v0.4.0

## References

- The entire class of "ephemeral research prototypes" makes this trade-off: fast iteration and simplicity over production readiness
- Redis (in-memory database with optional persistence) validates the approach of "memory-first, persistence-optional" for performance-critical workloads
