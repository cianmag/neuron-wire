# Neuron-Wire Documentation

[Introduction](README.md)

---

# Architecture
- [System Architecture (Diagrams)](ARCHITECTURE_DIAGRAMS.md)
- [Formal Architecture](ARCHITECTURE.md)
- [Protocol Specification](PROTOCOL_SPEC.md)
- [Architecture Decision Records](ADR/README.md)
  - [001: Single-Threaded Engine Loop](ADR/001-single-threaded-engine-loop.md)
  - [002: UDP Transport with Reliability Tiers](ADR/002-udp-transport-with-reliability-tiers.md)
  - [003: Kademlia Latency-Weighted KBuckets](ADR/003-kademlia-latency-weighted-kbuckets.md)
  - [004: Hebbian STDP not Backprop](ADR/004-hebbian-stdp-not-backprop.md)
  - [005: FlatBuffer Zero-Copy Serialization](ADR/005-flatbuffer-zero-copy-serialization.md)
  - [006: Sparse Gossip over Full Mesh](ADR/006-sparse-gossip-over-full-mesh.md)
  - [007: Deterministic Simulation Paper Mode](ADR/007-deterministic-simulation-paper-mode.md)
  - [008: No Persistent Storage — In Memory](ADR/008-no-persistent-storage-in-memory.md)

# Engineering
- [Developer Guide](DEVELOPER_GUIDE.md)
- [CI/CD Pipeline](DEVELOPER_GUIDE.md#7-ci-pipeline)
- [Testing Guide](DEVELOPER_GUIDE.md#6-testing)
- [Debugging Patterns](DEVELOPER_GUIDE.md#8-debugging-patterns)

# Research
- [Formal Mathematical Model](FORMAL_MODEL.md)
- [Research Paper](PAPER.md)
- [Foundational Q&A](FOUNDATIONAL_QNA.md)
- [Benchmark Results](ARCHITECTURE.md#9-benchmark-results)
- [Baseline Comparisons](ARCHITECTURE.md#10-baseline-comparisons)
- [Complexity Analysis](ARCHITECTURE.md#8-complexity-analysis)
- [Lessons Learned](LESSONS_LEARNED.md)

# Roadmap & Contribution
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)
- [Changelog](CHANGELOG.md)
- [Reproducibility](REPRODUCIBILITY.md)
- [Publishing Checklist](PUBLISHING_CHECKLIST.md)

# Reference
- [Rust API Docs](../api/neuron_wire/index.html)
- [Stats Summary](STATS.md)
- [License](LICENSE)
