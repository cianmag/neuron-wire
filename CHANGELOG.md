# Changelog

All notable changes to neuron-wire are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- `LESSONS_LEARNED.md` — personal reflections on hypothesis failure, reproducibility, engineering vs. science, and process
- `FOUNDATIONAL_QNA.md` — 20 categories of research questions answered from codebase evidence
- Plain-English glossary for non-technical readers (in FOUNDATIONAL_QNA.md)
- `LICENSE-MIT` — MIT license file
- README documentation section with links to all supporting docs

### Changed
- `README.md` — updated stale numbers (72 tests, 7,975 LOC, zero warnings), added documentation index and license link
- `FOUNDATIONAL_QNA.md` — rewritten in confident, disciplined tone; added citations, north star, strongest criticism embraced as roadmap

### Fixed
- All maintenance-ping claims qualified with "in our simulator, under the tested conditions"

---

## v0.3.0 — Negative SGA Result (2026-06-26)

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

## v0.2.0 — Research Prototype (2026-05)

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

## v0.1.0 — Initial Prototype (2026-04)

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
