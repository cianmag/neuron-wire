# Neuron Wire Protocol — Technical Blog Series

An engineering journal documenting the design, failures, lessons, and benchmarks of building a peer-to-peer neural runtime for distributed computation.

| # | Title | Content |
|---|-------|---------|
| 1 | [Design Decisions](01-design-decisions.md) | Why another DHT, epidemic broadcast vs iterative routing, single-threaded engine, zero-copy FlatBuffer wire format, K=20 latency-sorted buckets, apoptosis as memory management |
| 2 | [Failed Experiments](02-failed-experiments.md) | Apoptosis the node-killer, NWP frame offset miscalculation, broken duplicate detection, 50-node convergence gap, zero-copy over-engineering |
| 3 | [Lessons Learned](03-lessons-learned.md) | Verification-first development, determinism vs throughput, single-thread patterns, failure defaults, self-verifying benchmarks, the 80/20 of distributed debugging |
| 4 | [Benchmarks](04-benchmarks.md) | 50 nodes in 4 seconds, zero-variance convergence, failure injection results, adversarial attack resilience, baseline comparisons, reproducibility guarantee |

## Tone

Engineering journals, not marketing. All data reproducible via `cargo run --example simulate -- --seed 42 --paper-mode`.

## License

MIT / Apache 2.0 (same as the parent project).
