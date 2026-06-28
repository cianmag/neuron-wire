# Changelog

All notable changes to neuron-wire are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v0.3.0] — Quality Infrastructure (2026-06-28)

> Git tag: `v0.3.0`

### Added
- CI matrix: `test` runs on ubuntu / macos / windows × stable Rust
- CI: WASM compile check (`cargo build --target wasm32-wasip1`)
- CI: `cargo-semver-checks` API compatibility against latest git tag
- CI: Criterion benchmark baseline caching with `--load-baseline master` across PR branches
- CI: `cargo-llvm-cov` code coverage, `cargo-audit`, `cargo-deny` (all block on failure)
- 110 unit tests across 17 modules (+43% coverage from 77)
- 14 criterion benchmarks for hot paths (header, CRC, DHT, Hebbian, forward pass)
- Property-based tests via `proptest` (8 targets in `tests/proptest.rs`)
- Integration tests (7 roundtrip scenarios in `tests/integration.rs`)
- Fuzz targets in `fuzz/` for header parsing (`libfuzzer`)

### Changed
- `Cargo.toml`: version 0.2.0 → 0.3.0, MSRV `rust-version = "1.87"`
- `src/lib.rs`: `#![deny(missing_docs)]` enforced; 16 submodules annotated with `#![allow(missing_docs)]`
- `DEVELOPER_GUIDE.md`: CI pipeline described as 7 parallel jobs, test counts corrected

### Fixed
- Removed stray `mut` on `MessageHeader` in integration test (zero-warnings policy enforcement)

### Infrastructure
- `benches/benchmarks.rs` — throughput-measured CRC and parse benchmarks
- `tests/integration.rs` — end-to-end protocol roundtrips
- `tests/proptest.rs` — randomized DHT, Hebbian, header invariants
- `fuzz/` — libfuzzer target for header parsing
- `.github/workflows/ci.yml` — 7 parallel jobs with 3-OS matrix
- `deny.toml` — license & duplicate-dependency policy

---

## [v0.3.0-negative-sga] — Negative SGA Result (2026-06-26)

> GitHub release: `v0.3.0-negative-sga`

### Added
- `ROADMAP.md` — 20-direction research plan with priority matrix
- `PUBLISHING_CHECKLIST.md` — Level 1–5 readiness inventory
- SGA (Sparse Gradient Aging) benchmark suite (10, 25, 50 nodes)
- Benchmark report documenting the surprising finding
- 6 experiment directories with raw CSV output

### Changed
- Simulator supports `--paper-mode` with deterministic seeds and frozen configs

### Removed
- All pre-existing compiler warnings (zero-warnings policy)

### Research
- **Surprising finding:** Under tested conditions (stable LAN, saturated k-buckets, ≤300s runs), maintenance pings did not measurably improve routing quality over sending zero maintenance pings
- SGA increased bandwidth 1.9–2.45× across all node counts
- Routing convergence: 3.0s for all tested configurations (10, 25, 50 nodes)

---

## [v0.2.0] — Research Prototype (2026-05)

### Added
- Interactive DHT visualization and trace output system
- Adversarial testing framework: bad packets, corrupted state, spoofed identity, replay attacks
- Failure injection framework: packet filter, NodeDeath, Partition, MaliciousNode
- Formal architecture document (`ARCHITECTURE.md`, 1,621 lines)
- Reproducibility infrastructure (`REPRODUCIBILITY.md`, CI pipeline, validation script)
- Live benchmark dashboard on Vercel
- Multi-trial benchmark runner with LaTeX-compatible output
- 50-node DHT convergence benchmark: 49/49 peers in 4.0s

### Fixed
- Three bugs blocking all DHT message routing
- Event channel full errors
- CSV metrics now show real engine rx/tx/peers instead of zeros
- Full DHT convergence achieved for 5-node networks

### Infrastructure
- GitHub Actions CI: build + test + clippy + benchmark validation
- Deterministic simulation with fixed seeds
- Raw CSV output for all experiments
- Zero-warnings policy enforced via `cargo clippy -D warnings`

---

## [v0.1.0] — Initial Prototype (2026-04)

### Added
- Kademlia-style DHT routing with k-buckets (K=20, 256 buckets)
- Custom UDP transport with reliable queue and 3 reliability tiers
- Single-threaded engine loop with 6 phases
- Hebbian STDP learning system with sparse gossip
- Forward pass with tanh activation, prediction error, and surprise detection
- Neurogenesis system: surprise-driven neuron birth
- Apoptosis system: timer-based pruning with death spiral guard
- FlatBuffer-based zero-copy serialization
- Node lifecycle management with shutdown signal
- 72 unit tests across 15 modules
- 7,975 lines of Rust across 26 source files

### Notes
- Initial commit `e2d99a5`
- Design inspired by Kademlia DHT, FlatBuffers, Hebbian learning/STDP, and neuroplasticity concepts
- Dual-licensed MIT

[Unreleased]: https://github.com/cianmag/neuron-wire/compare/v0.3.0...HEAD
[v0.3.0]: https://github.com/cianmag/neuron-wire/compare/v0.3.0-negative-sga...v0.3.0
[v0.3.0-negative-sga]: https://github.com/cianmag/neuron-wire/compare/v0.2.0...v0.3.0-negative-sga
[v0.2.0]: https://github.com/cianmag/neuron-wire/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/cianmag/neuron-wire/releases/tag/v0.1.0

