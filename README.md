# Neuron Wire (NWP)

**Infrastructure layer for decentralized AI** — peer discovery, gradient exchange, and distributed learning over P2P networks without centralized coordination.

> ## Status: Validated Research Prototype
>
> **Validated through:** deterministic simulation · local multi-process networking ·
> property-based and fuzz testing · network impairment emulation · reproducible benchmarks
>
> **Not yet completed:** geographically distributed deployment · external security audit ·
> independent replication
>
> **Funding purpose:** build and evaluate the first real distributed test network
> (see [docs/EVIDENCE_REPORT.md](docs/EVIDENCE_REPORT.md) and [docs/GRANT_SUMMARY.md](docs/GRANT_SUMMARY.md))

[![CI](https://github.com/cianmag/neuron-wire/actions/workflows/ci.yml/badge.svg)](https://github.com/cianmag/neuron-wire/actions/workflows/ci.yml)
[![Evidence](https://github.com/cianmag/neuron-wire/actions/workflows/evidence.yml/badge.svg)](https://github.com/cianmag/neuron-wire/actions/workflows/evidence.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![Dashboard](https://img.shields.io/badge/dashboard-live-00c8ff)](https://neuron-wire-dashboard.vercel.app)
[![Documentation](https://img.shields.io/badge/docs-mdBook-00c853)](https://github.com/cianmag/neuron-wire)
[![Crates.io](https://img.shields.io/badge/crate-v0.3.0-blue)]()
[![Security](https://img.shields.io/badge/security-Ed25519%20signed-ff69b4)](SECURITY.md)

---

## Overview

Neuron Wire is an open-source Rust framework that enables any device reachable over a network to participate in collaborative AI without provisioning servers, trusting a central coordinator, or exposing private data.

The protocol provides: latency-weighted Kademlia DHT for peer discovery, custom reliable UDP transport with gradient decay, Hebbian STDP distributed learning, Ed25519 packet authentication with optional XChaCha20-Poly1305 encryption, trust-based rate limiting, and deterministic paper-mode simulation. A single-threaded non-blocking engine loop sustains ~400 KHz–1 MHz tick rates on commodity hardware with zero external runtime dependencies.

---

## Quick Start

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release   # opt-level=z, LTO, stripped binary
cargo test              # full test suite
```

### Run a DHT convergence benchmark

```bash
# v3 (100K nodes — real simulator)
cargo run --release --bin bench-fast

# v4 hybrid model (up to 1B nodes — statistical)
cargo run --release --bin bench-fast-v4 "100k,1m,10m,100m,1b"
```

### Run a simulation

```bash
cargo run --release --example simulate -- --paper-mode --nodes 10 --duration 120 \
    --output-dir results/experiment-001
```

---

## Key Results

| Scale | Converged | CT (sim‑s) | Avg Peers | Bandwidth | Wall Time |
|-------|-----------|------------|-----------|-----------|-----------|
| 100 K | ✅ 100% | 7.0 | 59 | 202 Mbps | 43 s |
| 1 M | ✅ 99.8% | 10.0 | 148 | 149 Mbps | 12 s |
| 10 M | ✅ 98.9% | 10.0 | 142 | 137 Mbps | 12 s |
| 100 M | ✅ 99.8% | 12.5 | 176 | 125 Mbps | 15 s |
| **1 B** | **✅ 99.5%** | **12.5** | **176** | **125 Mbps** | **16 s** |

*Hybrid model: 200 K active nodes + virtual target space for remainder. Convergence threshold = max(log₂(N) × 3, 30) peers.*

---

## Documentation

| Document | Content |
|----------|---------|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Subsystem design, engine loop, data flow, design decisions |
| [`ARCHITECTURE_DIAGRAMS.md`](ARCHITECTURE_DIAGRAMS.md) | Visual system architecture |
| [`ROADMAP.md`](ROADMAP.md) | Three deliverables with milestones |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to contribute |
| [`GRANT.md`](GRANT.md) | Grant application & research plan |
| [`FOUNDATIONAL_QNA.md`](FOUNDATIONAL_QNA.md) | FAQ for researchers |
| [`PROTOCOL_SPEC.md`](PROTOCOL_SPEC.md) | Wire format, message types, DHT routing |
| [`FORMAL_MODEL.md`](FORMAL_MODEL.md) | Convergence proofs, complexity bounds |
| [`SECURITY.md`](SECURITY.md) | Security posture, reporting, architecture, configuration |
| [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) | Reproduction guide |
| [`DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md) | Codebase tour, testing, profiling |
| [`PUBLISHING_CHECKLIST.md`](PUBLISHING_CHECKLIST.md) | Publication readiness |

API docs: `cargo doc --no-deps --open`

---

## Architecture (summary)

| Subsystem | Module | Description |
|-----------|--------|-------------|
| DHT Routing | [`dht.rs`](src/dht.rs) | Latency-weighted Kademlia, 256 buckets, K=20, DNS seeds |
| UDP Transport | [`transport.rs`](src/transport.rs) | Custom reliable UDP, 3 tiers, ACK bitfield, gradient decay |
| Engine Loop | [`engine_loop.rs`](src/engine_loop.rs) | Single-threaded non-blocking 6-phase tick, 400 KHz–1 MHz |
| Neural Compute | [`forward_pass.rs`](src/forward_pass.rs) | Activation propagation, tanh squashing, prediction error |
| Hebbian STDP | [`hebbian.rs`](src/hebbian.rs) | Spike-timing-dependent plasticity, weight decay, sparse gossip |
| Neurogenesis | [`neurogenesis.rs`](src/neurogenesis.rs) | Surprise-driven neuron birth (leaky accumulator) |
| Apoptosis | [`apoptosis.rs`](src/apoptosis.rs) | Programmed pruning: 4 criteria, death spiral guard |
| Identity & Trust | [`identity.rs`](src/identity.rs) | Ed25519 keypairs, packet signing + verification, trust scoring, Sybil resistance |
| Simulator | [`simulator.rs`](src/simulator.rs) | Deterministic paper-mode, metadata capture, known-good validation |

---

## Limitations (known)

1. **No NAT traversal** — all nodes must be directly reachable on UDP
2. **Encryption disabled by default** — `encrypt_payloads=false` until X25519 ECDH handshake is hardened
3. **Single-threaded engine** — cannot exploit multi-core
4. **No persistent storage** — RAM-only state (no snapshot/restore)
5. **DHT-only discovery** — no mDNS or LAN broadcast fallback
6. **Static gossip fanout** — fixed at 3 peers; may under-connect at 10³+ nodes
7. **No BFT consensus** — >⅓ malicious routing table nodes can partition
8. **Tested on Windows only** — UDP timing may vary on other platforms

*See [FOUNDATIONAL_QNA.md](FOUNDATIONAL_QNA.md) §Limitations for expanded discussion.*

---

## Contributing

Pull requests are welcome. All contributions must pass:
- `cargo fmt --check`
- `cargo clippy -- -D warnings` (zero-warnings policy)
- `cargo test`

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for detailed guidelines.

---

## License

[MIT](LICENSE-MIT) © 2025–2026 Zylvon & Neuron Wire Contributors

---

*Built with Rust 2021 edition. No async runtime. No dependency beyond std.*
