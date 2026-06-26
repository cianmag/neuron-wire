# Contributing to Neuron-Wire

Thanks for your interest. This is a research project, not a product. Contributions that move the research forward are valued over contributions that expand features.

---

## Quick Start

```bash
# Clone and build
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release

# Run tests (72 tests, must all pass)
cargo test

# Check for warnings (zero-warnings policy)
cargo clippy -D warnings

# Run a paper-mode experiment
cargo run --example simulate -- --paper-mode --nodes 10 --duration 30
```

---

## What Kinds of Contributions Help

This project is at the stage where **stronger evidence** adds more value than **more features**. Prioritized contribution areas from the roadmap:

1. **Real WAN deployment** — deploy 20–50 nodes across regions, publish results (this is the single most impactful thing anyone can do)
2. **Statistical analysis** — confidence intervals, effect sizes, multi-trial experiments
3. **Baseline comparisons** — benchmark against parameter server, federated averaging, decentralized SGD, vanilla gossip
4. **Mathematics** — formal derivations for communication complexity, gossip convergence, graph growth bounds
5. **Performance profiling** — CPU, RAM, cache misses, lock contention, serialization cost
6. **Security** — authentication, signatures, replay protection, Sybil resistance
7. **Visualization** — live dashboards for DHT state, neuron graph, packet flow, prediction error
8. **Documentation** — protocol specification, wire format doc, API reference, ADRs

If you're unsure where to start, open an issue with your background and interests.

---

## How to Contribute

### 1. Find or Open an Issue

Check [existing issues](https://github.com/cianmag/neuron-wire/issues) or open a new one describing what you want to work on. For research contributions (experiments, analysis, benchmarks), include your hypothesis and methodology in the issue.

### 2. Fork and Branch

```bash
git checkout -b feature/your-description
```

### 3. Make Your Changes

**For code changes:**
- Follow existing code style (run `cargo fmt` before committing)
- Add tests for new functionality
- Ensure zero warnings (`cargo clippy -D warnings`)
- All 72 tests must pass (`cargo test`)

**For documentation changes:**
- Write for a reader who may not know the field (see the glossary in FOUNDATIONAL_QNA.md for the style)
- If you add a strong claim, cite evidence
- If you document a limitation, be specific about what isn't tested

**For experiments:**
- Use `--paper-mode` with a fixed seed
- Output to `results/your-experiment-name/`
- Include raw CSV data, not just summary statistics
- Document the exact command, seed, config, and environment used

### 4. Commit

Use conventional commit messages:

```
feat:     new feature
fix:      bug fix
docs:     documentation
experiment: research experiment or benchmark
chore:    maintenance, refactoring, dependencies
```

### 5. Open a Pull Request

- Reference the issue your PR addresses
- Describe what was tested and how
- For experimental results, include reproduction instructions
- Keep PRs focused — one logical change per PR

---

## Research Contribution Guidelines

If you're contributing research results (experiments, benchmarks, analysis):

1. **Reproducibility is mandatory.** Every result must be traceable to a specific commit, seed, config, and environment.
2. **Negative results are welcome.** If your experiment disproves a hypothesis, that's a valuable contribution. Document what you expected, what happened, and why the discrepancy matters.
3. **Document limitations.** If your experiment only tested one configuration or one environment, say so. Trust is built by what you admit.
4. **Raw data over summaries.** Check in the raw CSV/JSON. Summaries and visualizations are nice, but the raw data is what allows others to verify and extend your work.

---

## Code of Conduct

This project is a research artifact, not a social platform. Be respectful, constructive, and evidence-based. Harassment, personal attacks, and promotion of unrelated products or services will not be tolerated.

---

## Questions?

Open a [GitHub Discussion](https://github.com/cianmag/neuron-wire/discussions) or email the maintainer at ci@cianmag.com.
