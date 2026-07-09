# Contributing to Neuron Wire

Thank you for your interest. This is a research infrastructure project. Contributions that strengthen the evidence base are valued over those that expand feature surface.

---

## Quick Start

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release
cargo test                    # all tests must pass
cargo clippy -- -D warnings   # zero-warnings policy
```

---

## What to Work On

Priority areas from the [roadmap](ROADMAP.md):

1. **Real WAN deployment** — deploy 20–50 nodes across regions, publish results
2. **Baseline comparisons** — benchmark against parameter server, federated averaging, gossip SGD
3. **Statistical analysis** — confidence intervals, effect sizes, multi-trial experiments
4. **Formal analysis** — communication complexity, gossip convergence, graph growth bounds
5. **Performance profiling** — CPU, RAM, cache, serialization cost
6. **Security** — authentication, replay protection, Sybil resistance
7. **Documentation** — protocol spec, API reference, tutorials
8. **Visualization** — live dashboards for DHT state, neural graph, packet flow

For newcomers, check [good first issues](https://github.com/cianmag/neuron-wire/issues?q=is:issue+is:open+label:"good+first+issue").

---

## How to Contribute

### 1. Open an Issue

Use the appropriate template:
- [Bug report](.github/ISSUE_TEMPLATE/bug_report.md)
- [Feature request](.github/ISSUE_TEMPLATE/feature_request.md)
- [Research contribution](.github/ISSUE_TEMPLATE/research_contribution.md)

For research contributions, include hypothesis and methodology.

### 2. Fork and Branch

```bash
git checkout -b feat/description   # new features
git checkout -b fix/description    # bug fixes
git checkout -b docs/description   # documentation
git checkout -b experiment/name    # research experiments
```

### 3. Set Up Pre-Commit Hooks

```bash
git config core.hooksPath .githooks
```

This runs `cargo fmt --check` and `cargo clippy` before every commit.

### 4. Make Changes

**Code:**
- Run `cargo fmt` before committing
- Add tests for new functionality
- Maintain zero warnings: `cargo clippy -- -D warnings`
- All tests must pass: `cargo test`

**Experiments and benchmarks:**
- Use `--paper-mode` with a fixed seed
- Output to `results/your-experiment-name/`
- Include raw CSV data, not just summaries
- Document exact command, seed, config, and environment

**Documentation:**
- Write for readers unfamiliar with the domain
- Claims require evidence; limitations require specificity

### 5. Commit

Use conventional commits:

```
feat:        new feature
fix:         bug fix
docs:        documentation
experiment:  research experiment or benchmark
chore:       maintenance, refactoring, dependencies
```

### 6. Open a Pull Request

- Reference the issue: `Closes #42`
- Describe what was tested and how
- Include reproduction instructions for experimental results
- One logical change per PR

### 7. Review Process

1. Open PR from fork/branch
2. CI runs: formatting, clippy, build, test, doc build
3. Maintainer review within 1–3 business days
4. Address feedback
5. Squash-merge to master

---

## Research Contribution Guidelines

1. **Reproducibility is mandatory.** Every result must be traceable to a specific commit, seed, config, and environment.
2. **Negative results are welcome.** If your experiment disproves a hypothesis, document what you expected, what happened, and why the discrepancy matters.
3. **Document limitations.** Trust is built by what you admit.
4. **Raw data over summaries.** Check in the raw CSV/JSON.
5. **Statistical rigor.** Report effect sizes, confidence intervals, and trial counts.

---

## Project Structure

```
neuron-wire/
├── src/                   # Crate source
│   ├── components/        # EntityId, Neuron/Synapse (ECS)
│   ├── dht/              # Kademlia DHT with latency-weighted KBuckets
│   ├── ml/               # Hebbian/STDP learning
│   ├── observability/    # Metrics, Prometheus, OTel, Dashboard
│   ├── transport/        # UDP transport with reliability tiers
│   ├── engine_loop.rs    # Main tick-based engine
│   └── simulator.rs      # Deterministic simulation framework
├── examples/             # Runnable examples
├── tests/                # Integration, stress, property-based tests
├── benches/              # Criterion benchmarks
├── fuzz/                 # cargo-fuzz targets
├── docs/                 # mdBook documentation source
├── ADR/                  # Architecture Decision Records
├── website/              # Project landing page
└── .github/              # CI workflows and issue templates
```

---

## Community

- **Discord:** [Join](https://discord.gg/neuron-wire)
- **GitHub Discussions:** [Start a discussion](https://github.com/cianmag/neuron-wire/discussions)
- **Code of Conduct:** [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- **Email:** [team@zylvon.com](mailto:team@zylvon.com)
