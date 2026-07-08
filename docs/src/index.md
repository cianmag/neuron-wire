# Neuron-Wire Documentation

> **Zero-Infrastructure AI** — open-source infrastructure layer for decentralized AI systems.

Welcome to the Neuron Wire Protocol (NWP) documentation. This book covers everything from getting started with your first node to contributing research experiments.

## Quick Start

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release
cargo run --example simulate -- --nodes 5 --duration 25 --seed 42
```

## What's Inside

| Section | Description |
|---------|-------------|
| [Tutorials](tutorials/README.md) | Step-by-step guides: first node, multi-node network, DHT, experiments |
| [Architecture](ARCHITECTURE.md) | Formal subsystem design, engine loop, data flow, decisions |
| [Developer Guide](DEVELOPER_GUIDE.md) | Codebase tour, testing, debugging, profiling |
| [Research](FORMAL_MODEL.md) | Mathematical model, paper outline, foundational Q&A |
| [Roadmap](ROADMAP.md) | 20 research directions with effort estimates |
| [Contributing](CONTRIBUTING.md) | How to contribute code, experiments, and documentation |

## Project Status

- **Version:** v0.3.0 (Quality Infrastructure)
- **Lines of Rust:** ~8,150
- **Tests:** 110+ unit tests, 14 benchmarks, 7 integration tests, 8 proptest targets
- **CI:** 8 GitHub Actions jobs (test matrix, docs, audit, benchmarks, release)
- **Documentation:** This mdBook + auto-generated `cargo doc` + 6 tutorials + 8 ADRs

## License

MIT © 2025–2026 Zylvon & Neuron Wire Contributors

*Built with Rust 2021 edition. Single-threaded, non-blocking, and intentionally free of async runtimes.*
