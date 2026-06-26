# neuron-wire: Publishing Readiness Checklist

> Generated 2026-06-26 from full project inventory

---

## Level 1: Must Have (Finish Before Publishing)

### Core Runtime

| Item | Status | Evidence |
|------|--------|----------|
| Distributed runtime | ✅ | `engine_loop.rs` — multi-node async engine |
| DHT routing | ✅ | `src/dht.rs` — Kademlia-style k-buckets, 14 tests |
| UDP transport | ✅ | `src/udp_transport.rs` + `src/transport.rs` (reliable layer) |
| Reliable packet layer | ✅ | `reliable_queue` in `transport.rs` — ACK/retransmission |
| Node lifecycle management | ✅ | `Node` struct with shutdown signal, engine_stats, packet_filter |
| Dynamic graph expansion | ✅ | `neurogenesis.rs` — spawns neurons on prediction novelty |
| Weight adaptation engine | ✅ | `hebbian.rs` — STDP learning, gossip, gradient accumulation |
| Prediction error integrator | ✅ | `forward_pass.rs` — surprise-driven learning |
| Sparse gossip | ✅ | `hebbian.rs:gossip_target_selection` — mutation-based selection |
| **End-to-end learning demo** | ⬜ | CORE GAP — no single command that shows "before vs after" learning |

### Testing

| Item | Status | Notes |
|------|--------|-------|
| Unit tests | ✅ | 72 tests across 15 modules |
| Integration tests | ⬜ | No `tests/` directory — all tests are in-module |
| Network tests | ⬜ | Simulator runs localhost only — no `#[cfg(network)]` tests |
| Failure tests | ✅ | `adversary.rs` — 9 tests (corruption, replay, garbage) |
| Fuzz tests | ⬜ | No `fuzz/` directory, no cargo-fuzz setup |
| Property-based tests | ⬜ | No proptest/quickcheck dependency |
| Regression tests | ⬜ | No `tests/regression/` — known-good comparison exists in CI but only validates benchmarks |
| 90%+ code coverage | ⬜ | No coverage tool configured |
| CI runs every test | ✅ | `.github/workflows/ci.yml` — build + test + clippy + fmt + benchmark validation |

### Documentation

| Item | Status | Notes |
|------|--------|-------|
| Architecture document | ✅ | `ARCHITECTURE.md` — 1621 lines, 9 major sections |
| API reference | ✅ | `cargo doc` — auto-generated from doc comments |
| Packet format specification | ✅ | `lib.rs` wire format comment + `ARCHITECTURE.md §3.1` |
| State machine diagrams | ✅ | Mermaid diagrams in `ARCHITECTURE.md` (retransmission, DHT message handling) |
| Protocol specification | ✅ | `ARCHITECTURE.md §3-6` covers NWP wire format, frame layout |
| Design rationale | ✅ | `ARCHITECTURE.md §1.3` — async vs threaded rationale |
| Failure modes | ✅ | `ARCHITECTURE.md §7` — 9 failure scenarios |
| Threat model | ✅ | `ARCHITECTURE.md §7.7` adversary section |

### Benchmarks

| Item | Status | Notes |
|------|--------|-------|
| CPU | ⬜ | Not measured per trial |
| RAM | ⬜ | Not measured |
| Bandwidth | ✅ | `BENCHMARK_REPORT.md` — fixed vs SGA at 10/25/50 nodes |
| Latency | ⬜ | Not measured |
| Tick jitter | ⬜ | Not measured |
| Packet loss | ✅ | Adversary tests inject loss |
| Node churn | ⬜ | No churn model benchmarked |
| Recovery time | ✅ | TrialResult records post-failure convergence |
| Routing latency | ⬜ | DHT lookup latency not measured |
| Message count | ✅ | Packets_recv/sent in every trial output |
| Scalability | ✅ | 10/25/50 node sweeps |

### Gaps to Close Before Publishing

1. **End-to-end learning demo** — need a `--demo` mode or example that shows learning progression (before vs after weights, prediction error dropping over time)
2. **Integration tests** — add `tests/` directory with multi-node convergence tests
3. **Coverage** — configure `cargo-tarpaulin` or `cargo-llvm-cov`, enforce in CI
4. **CPU/RAM benchmarks** — instrument the simulator tick loop with timing and memory tracking

---

## Level 2: Research Quality

### Mathematical Analysis

| Item | Status | Notes |
|------|--------|-------|
| Time complexity | ⬜ | No Θ/Ω analysis anywhere |
| Space complexity | ⬜ | No per-node/per-packet space analysis |
| Network complexity | ⬜ | No message count model |
| Routing complexity | ⬜ | No DHT lookup hop analysis |
| Message complexity | ⬜ | No per-cycle message bound |
| Bandwidth model | ⬜ | No O() formula for bandwidth scaling |

### Baselines

| Baseline | Status | Notes |
|----------|--------|-------|
| Centralized runtime | ⬜ | No comparison |
| Standard gossip | ⬜ | No comparison |
| Random routing | ⬜ | No comparison |
| Static graph | ⬜ | No comparison |
| Federated averaging | ⬜ | No comparison |
| Simple parameter server | ⬜ | No comparison |

### Reproducibility

| Item | Status | Notes |
|------|--------|-------|
| One-command run | ✅ | `--paper-mode --config experiments/figure4.toml` (but no `experiments/` dir yet) |
| CSV output | ✅ | `summary.csv` + `convergence.csv` |
| PNG output | ⬜ | No chart generation |
| JSON metadata | ✅ | Full config + parameters |
| Logs | ✅ | `eprintln!` with tick-level stats |
| Config copy | ✅ | `experiment.toml` snapshot |

### Statistical Analysis

| Item | Status | Notes |
|------|--------|-------|
| Mean | ✅ | Reported |
| Median | ⬜ | Not computed |
| Standard deviation | ⬜ | Only computed with >1 trial |
| Confidence interval | ⬜ | Not computed |
| Trial count | ✅ | User-configured |
| Random seeds | ✅ | Sequential seeds per trial |

### Threats to Validity

| Item | Status |
|------|--------|
| Simulator assumptions | ⬜ | No discussion |
| WAN differences | ⬜ | No treatment |
| Synthetic workload | ⬜ | No realism analysis |
| Scale limits | ⬜ | No scaling ceiling |
| Fixed parameters | ⬜ | No parameter sensitivity |
| Future work | ⬜ | No section |

---

## Level 3: Engineering Excellence

### CI/CD

| Item | Status | Notes |
|------|--------|-------|
| Build | ✅ | |
| Test | ✅ | |
| Lint | ✅ | `cargo fmt --check` + `cargo clippy` |
| Clippy | ✅ | With `-D warnings` |
| Rustfmt | ✅ | |
| Coverage | ⬜ | No coverage gate |
| Benchmark validation | ✅ | Known-good CSV comparison |

### Docker & Deployment

| Item | Status |
|------|--------|
| Docker image | ⬜ |
| Docker Compose | ⬜ |
| Kubernetes example | ⬜ |

### Cross-platform

| Item | Status |
|------|--------|
| Windows | ⬜ (CI doesn't test) |
| Linux | ✅ |
| macOS | ⬜ (CI doesn't test) |

### Config System

| Item | Status |
|------|--------|
| Learning rate | ⬜ (no config knob) |
| Gossip interval | ✅ (`gossip_interval_ticks`) |
| Packet size | ⬜ (not configurable) |
| Node count | ✅ (`--nodes`) |
| Thresholds | ⬜ (varies by subsystem) |
| Timeouts | ✅ (`--stale-ping-secs`) |
| **No magic numbers** | ⬜ **Major gap** — `MAX_PEERS_PER_BUCKET`, `STALE_PING_S`, retry constants are hardcoded in `dht.rs` and `transport.rs` |

### Logging

| Item | Status |
|------|--------|
| Structured logs (JSON) | ⬜ |
| Human readable | ✅ (`eprintln!`) |
| Debug mode | ⬜ |
| Metrics mode | ⬜ |

---

## Level 4: Research Differentiators

### Live Dashboard

| Item | Status |
|------|--------|
| Active nodes | ⬜ |
| Connections | ⬜ |
| RTT | ⬜ |
| Gossip traffic | ⬜ |
| CPU/RAM | ⬜ |
| Packet loss | ⬜ |
| Graph topology | ⬜ (`tools/visualizer/` is static HTML — not live) |

### Visualization

| Item | Status |
|------|--------|
| Interactive network graph | ⬜ (static HTML only) |
| Routing | ⬜ |
| Edge weights | ⬜ |
| Prediction error | ⬜ |
| Node failures | ⬜ |
| Recovery | ⬜ |

### Failure Injection

| Item | Status |
|------|--------|
| Node crashes | ✅ |
| Packet loss | ✅ |
| Latency spikes | ⬜ |
| Network partitions | ✅ |
| Byzantine nodes | ✅ |
| Corrupted packets | ✅ |

### Real WAN Deployment

| Item | Status |
|------|--------|
| Asia | ⬜ |
| Europe | ⬜ |
| North America | ⬜ |
| South America | ⬜ |

### Open-source Quality

| Item | Status |
|------|--------|
| CONTRIBUTING.md | ⬜ |
| CODE_OF_CONDUCT.md | ⬜ |
| LICENSE | ⬜ |
| SECURITY.md | ⬜ |
| CHANGELOG.md | ⬜ |
| Roadmap | ⬜ |

---

## Level 5: Things That Make It Exceptional

### Technical Website

| Item | Status |
|------|--------|
| Website | ⬜ (vercel domain exists, nothing deployed?) |
| Interactive architecture | ⬜ |
| Benchmarks | ⬜ |
| Papers | ⬜ |
| Blog | ⬜ |
| Documentation | ⬜ |
| Demo | ⬜ |

### Blog Series

| Item | Status |
|------|--------|
| Why this runtime | ⬜ |
| Why gossip vs sync | ⬜ |
| Lessons from reliable UDP | ⬜ |
| Failure injection results | ⬜ |
| Disproved assumptions | ⬜ |

### Research Paper

| Item | Status |
|------|--------|
| Runtime architecture paper | ✅ (`PAPER.md`) |
| Dynamic graph expansion paper | ⬜ |
| Sparse gossip paper | ⬜ |
| Node specialization paper | ⬜ |

### External Validation

| Item | Status |
|------|--------|
| GitHub contributors | ⬜ (1 person) |
| Issues from users | ⬜ |
| Pull requests | ⬜ |
| Professor feedback | ⬜ |
| Research citations | ⬜ |

### Community

| Item | Status |
|------|--------|
| Discord | ⬜ |
| Discussions | ⬜ (GitHub Discussions?) |
| Wiki | ⬜ |
| Example projects | ⬜ |

### Bonus Features

| Item | Status |
|------|--------|
| WebAssembly demo | ⬜ |
| Interactive simulator | ⬜ |
| Packet inspector | ⬜ |
| Benchmark comparison tool | ⬜ |
| Graph replay viewer | ⬜ |
| Metrics exporter (Prometheus) | ⬜ |
| REST/gRPC API | ⬜ |
| Python bindings | ⬜ |
| Rust SDK | ✅ (it's a library) |
| C API | ⬜ |

---

## Summary

| Level | Status | Next Action |
|-------|--------|-------------|
| **L1: Must Have** | ~65% done | End-to-end demo, integration tests, coverage, CPU/RAM benchmarks |
| **L2: Research Quality** | ~20% done | Complexity analysis, baselines, threats to validity |
| **L3: Engineering** | ~30% done | No magic numbers, Docker, cross-platform CI, structured logging |
| **L4: Differentiators** | ~15% done | LICENSE + CHANGELOG first, live dashboard second |
| **L5: Exceptional** | ~5% done | Website + blog series most impactful per effort |

---

## Full Roadmap

See [`ROADMAP.md`](ROADMAP.md) for the complete 20-direction plan with:

- Detailed breakdown of every item (testbed to protocol discovery)
- Effort estimates (1 week to 12 months)
- Priority matrix sorted by impact/effort ratio
- Immediate next actions
