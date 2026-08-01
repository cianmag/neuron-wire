# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] — 2026-08-01

### Added

#### Release & Evidence (v0.3.1)
- **v0.3.1 evidence release** — all experimental numbers pinned to commit `265e216`
  (342 tests, E1–E9 matrix, clusters, emulation, distributed-learning E2E); evidence run
  `30697662078` green. Tag `v0.3.1`.
- **Two-page technical summary** — `docs/TECHNICAL_SUMMARY.md` for grant reviewers.
- **Submission-ready grant drafts** — `docs/GRANT_SUBMISSIONS/PROTOCOL_LABS.md`
  (active program) and `docs/GRANT_SUBMISSIONS/NLNET_OIS.md` (post-summer reopen).
- **Release tooling** — `release/` scripts: grant-package PDF builder, demo-slide
  generator, narration, and video assembly (artifacts attached to the v0.3.1 release).
- **Fixed release workflow** — the packaged binary is `node` (auto-detected from
  `src/bin/node.rs`), not `neuron-wire`; the previous packaging step failed on every
  tag push.

## [Unreleased]

### Added

#### Engine & Core
- **Engine loop rebuild** — 1,434-line engine loop with full security hardening
- **Trust-based rate limiting** with convergence proof (formal specification)
- **Heartbeat protocol** — 30s keepalive for connection health
- **Connection limits** — `TOO_MANY_PEERS` rejection at capacity
- **Peer eviction** — 5-minute TTL with automatic cleanup

#### Observability
- **Structured JSON logging** — machine-readable logs for production monitoring
- **Prometheus metrics** — 28+ metrics covering network, crypto, and learning
- **Dashboard** — 4 time-series charts for real-time observability

#### Testing & Validation
- **Property-based crypto tests** — 4 proptests covering key exchange, signing, and encryption
- **Stress test suite** — 9 tests for high-load and edge-case scenarios
- **Fuzzing setup** — `cargo-fuzz` integration for continuous fuzz testing
- **CI pipeline** — 5-job GitHub Actions workflow (build, test, lint, fuzz, security)

#### Documentation
- **Algorithm specification** — formal proofs for trust convergence and rate limiting
- **Architecture diagrams** — 6 Mermaid.js diagrams covering system topology
- **Dashboard deployment** — Vercel-hosted observability dashboard

#### Infrastructure
- **Docker Compose production stack** — ready-to-deploy container orchestration

## [0.3.0] - 2026-07-24

### Added
- Hybrid DHT simulator supporting 200k active + virtual nodes
- 1B node convergence milestone (scales 100K–1B, <16s convergence)
- Fixed `while loop i+=1`, round-robin periodic, FIND_NODE relay
- MAX_PEERS=500, k-bucket cap, 5 scaling tiers
- WASM browser demo (Tier 10 — P2P Neural Network Demo)
- Full security layer (Tier 5) with Ed25519/X25519, ChaCha20-Poly1305
- Observability suite with structured logging
- Open-source community infrastructure (Tier 6)
- Reproducibility infrastructure + 10-paper series (Tier 7 & 8)
- Governing Principles documentation

### Changed
- Reframed as "Zero-Infrastructure AI" — infrastructure layer for decentralized AI

## [0.2.0] - 2026-07-24

### Added
- Security subsystem wired into EngineLoop
- Research questions framework with five governing rules
- 10-tier improvement framework
- Research Brief for grant reviewers
- Grant incorporation with strategic refinements

## [0.1.0] - 2026-07-24

### Added
- Initial release of neuron-wire
- P2P peer discovery via Kademlia DHT
- Gradient exchange protocol
- Basic distributed learning primitives
- Demo crate with WASM target

[Unreleased]: https://github.com/cianmag/neuron-wire/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/cianmag/neuron-wire/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/cianmag/neuron-wire/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cianmag/neuron-wire/releases/tag/v0.1.0
