# Release Notes

> Reverse chronological log of neuron-wire releases.
>
> For a detailed per-change breakdown, see [CHANGELOG.md](CHANGELOG.md).

---

## [v0.3.0] — Quality Infrastructure

**Release date:** 2026-06-28  
**Git tag:** `v0.3.0`  
**Commit range:** [`v0.3.0-negative-sga...v0.3.0`](https://github.com/cianmag/neuron-wire/compare/v0.3.0-negative-sga...v0.3.0)

### Highlights

- CI/CD maturation: cross-platform test matrix, WASM compilation checking, API compatibility (semver-checks), benchmark regression detection, code coverage, audit, and license deny — all blocking on failure.
- Documentation infrastructure: full mdBook docs site deployed to GitHub Pages, with auto-generated `cargo doc` and Mermaid architecture diagrams.
- Test coverage expanded from 77 to 110 unit tests, plus 14 criterion benchmarks, 7 integration tests, 8 proptest targets, and fuzz targets.

### What's New

- **CI matrix** — `test` job runs on ubuntu / macos / windows × stable Rust
- **WASM compile check** — `cargo build --target wasm32-wasip1` as a CI gate
- **API compatibility** — `cargo-semver-checks` against latest git tag to catch breaking changes
- **Benchmark regression detection** — Criterion baseline caching; CI fails if any benchmark regressed >5%
- **Code coverage** — `cargo-llvm-cov` with coverage threshold enforcement
- **Security audit** — `cargo-audit` blocks on vulnerabilities
- **License & dependency policy** — `cargo-deny` checks for duplicate deps and license compatibility
- **Unused dependency detection** — `cargo machete` in CI
- **Release workflow** — auto-creates GitHub Release and optionally publishes to crates.io on `v*.*.*` tag push
- **Documentation website** — mdBook at `docs/` with all developer docs + auto-generated `cargo doc`, deployed via `docs.yml`
- **Stress/soak tests** — `tests/stress.rs` with 30s ping-pong, 10s many-nodes, 60s soak
- **Architecture diagrams** — 8 Mermaid diagrams covering DHT routing, learning pipeline, packet flow, state machine, etc.
- **Pre-commit hook** — `.githooks/pre-commit` enforces formatting + clippy

### Breaking Changes

- Minimum supported Rust version (MSRV) set to **1.87** (`rust-version = "1.87"` in `Cargo.toml`)
- `#![deny(missing_docs)]` enforced at crate root; 16 submodules needed `#![allow(missing_docs)]` annotations

### Migration Notes

- Update your Rust toolchain: `rustup update stable`
- If you use `cargo build` directly, ensure your toolchain is ≥ 1.87
- No API or wire-format breaking changes

---

## [v0.2.0] — Research Prototype

**Release date:** 2026-05  
**Git tag:** `v0.2.0`  
**Commit range:** [`v0.1.0...v0.2.0`](https://github.com/cianmag/neuron-wire/compare/v0.1.0...v0.2.0)

### Highlights

- Full DHT convergence achieved: 49/49 peers in 4.0 seconds (50-node network)
- Formal architecture document (1,621 lines) covering every subsystem
- Adversarial testing framework for corruption, replay, node death, partition, and malicious behavior
- Reproducibility infrastructure with CI validation

### What's New

- **Interactive DHT visualization** — trace output system with live topology view
- **Adversarial testing** — `adversary.rs` module for injecting bad packets, corrupted state, spoofed identity, and replay attacks
- **Failure injection** — packet filter, `NodeDeath`, `Partition`, `MaliciousNode` injectors
- **Architecture document** — `ARCHITECTURE.md` with 1,621 lines of formal architecture description
- **Reproducibility** — `REPRODUCIBILITY.md`, CI pipeline validation script
- **Live benchmark dashboard** — deployed on Vercel
- **Multi-trial benchmark runner** — LaTeX-compatible output
- **50-node DHT convergence** — 49/49 peers discovered in 4.0 seconds

### Fixed

- Three bugs blocking all DHT message routing
- Event channel full errors under load
- CSV metrics now report real engine rx/tx/peers instead of zeros

### Breaking Changes

- None. Backward-compatible with v0.1.0 wire format.

### Migration Notes

- No migration steps required.

---

## [v0.1.0] — Initial Prototype

**Release date:** 2026-04  
**Git tag:** `v0.1.0`  
**Initial commit:** `e2d99a5`

### Highlights

- First working prototype of the neuron-wire P2P protocol
- Functional DHT routing, neural computation, and learning subsystems
- 7,975 lines of Rust across 26 source files

### What's New

- **Kademlia-style DHT routing** — k-buckets (K=20, 256 buckets) with full NodeId space
- **Custom UDP transport** — reliable message queue with 3 delivery tiers
- **Single-threaded engine loop** — 6-phase tick architecture (recv, route, compute, gossip, maintain, send)
- **Hebbian STDP learning** — sparse gossip-based weight updates with spike-timing-dependent plasticity
- **Forward pass** — tanh activation, prediction error computation, and surprise detection
- **Neurogenesis** — surprise-driven neuron birth (new neurons created when prediction error exceeds threshold)
- **Apoptosis** — timer-based pruning with death spiral guard to prevent cascade failures
- **FlatBuffer serialization** — zero-copy wire format for all message types
- **Node lifecycle management** — graceful shutdown with signal handling
- **72 unit tests** across 15 modules

### Design Influences

- Kademlia DHT for distributed routing
- FlatBuffers for zero-copy serialization
- Hebbian learning / STDP for local weight updates
- Neuroplasticity concepts (neurogenesis, apoptosis) for adaptive network topology

### Breaking Changes

- None. This is the initial release.

### Migration Notes

- Not applicable — first release.

---

[v0.3.0]: https://github.com/cianmag/neuron-wire/releases/tag/v0.3.0
[v0.2.0]: https://github.com/cianmag/neuron-wire/releases/tag/v0.2.0
[v0.1.0]: https://github.com/cianmag/neuron-wire/releases/tag/v0.1.0
