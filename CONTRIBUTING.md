# Contributing to Neuron-Wire

Thanks for your interest. This is a research project, not a product. Contributions that move the research forward are valued over contributions that expand features.

---

## Quick Start

```bash
# Clone and build
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release

# Run tests (110+ tests, must all pass)
cargo test

# Check for warnings (zero-warnings policy)
cargo clippy -- -D warnings

# Run benchmarks (requires nightly or bench feature)
cargo bench --features bench

# Run property-based tests
cargo test --features proptest

# Run examples
cargo run --example simulate -- --nodes 5 --duration 25 --seed 42
cargo run --example dashboard
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

If you're unsure where to start, check [good first issues](https://github.com/cianmag/neuron-wire/issues?q=is:issue+is:open+label:"good+first+issue") or open a discussion with your background and interests.

---

## How to Contribute

### 1. Find or Open an Issue

- **Bug reports:** Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md).
- **Feature requests:** Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).
- **Research contributions:** Use the [research contribution template](.github/ISSUE_TEMPLATE/research_contribution.md).
- **Good first issues:** Look for the `good first issue` label — these are curated for newcomers.

For research contributions (experiments, analysis, benchmarks), include your hypothesis and methodology in the issue.

### 2. Fork and Branch

```bash
# Always branch from master
git checkout -b feature/your-description
```

Branch naming convention:
- `feat/` — new features (`feat/udp-reliability-tier`)
- `fix/` — bug fixes (`fix/dht-convergence-race`)
- `docs/` — documentation (`docs/tutorial-dht-routing`)
- `experiment/` — research experiments (`experiment/wan-latency-benchmark`)
- `chore/` — maintenance (`chore/update-deps`)

### 3. Set Up Your Environment

Enable pre-commit hooks to catch issues early:

```bash
git config core.hooksPath .githooks
```

This runs `cargo fmt --check` and `cargo clippy` before every commit. If the checks fail, fix the issues and try again.

### 4. Make Your Changes

**For code changes:**
- Follow existing code style (run `cargo fmt` before committing)
- Add tests for new functionality (unit and property-based where appropriate)
- Ensure zero warnings (`cargo clippy -- -D warnings`)
- All 110+ tests must pass (`cargo test`)

**For fuzzing:**
- Install cargo-fuzz (requires nightly) and run `cargo +nightly fuzz run header_parse`
- The fuzz target lives in `fuzz/fuzz_targets/header_parse.rs`
- See `fuzz/README.md` for details

**For documentation changes:**
- Write for a reader who may not know the field (see the glossary in FOUNDATIONAL_QNA.md for the style)
- If you add a strong claim, cite evidence
- If you document a limitation, be specific about what isn't tested

**For experiments:**
- Use `--paper-mode` with a fixed seed
- Output to `results/your-experiment-name/`
- Include raw CSV data, not just summary statistics
- Document the exact command, seed, config, and environment used

### 5. Commit

Use conventional commit messages:

```
feat:        new feature
fix:         bug fix
docs:        documentation
experiment:  research experiment or benchmark
chore:       maintenance, refactoring, dependencies
```

Example good commit messages:
```
feat(dht): add latency-weighted KBucket eviction
fix(transport): handle partial UDP reads correctly
docs(tutorial): add multi-node networking tutorial
experiment(convergence): benchmark DHT with 50 nodes over 120s
chore(deps): update ed25519-dalek to v2.1
```

### 6. Open a Pull Request

- Reference the issue your PR addresses (e.g., `Closes #42`)
- Describe what was tested and how
- For experimental results, include reproduction instructions
- Keep PRs focused — one logical change per PR
- Assign the `needs-review` label after opening

The CI pipeline will automatically:
1. Check formatting (`cargo fmt --check`)
2. Run lints (`cargo clippy -- -D warnings`)
3. Build the crate (`cargo build`)
4. Run tests (`cargo test`)
5. Build documentation (`cargo doc`)
6. Check for unused dependencies (`cargo machete`)

All CI checks must pass before review.

### 7. Code Review Process

1. **Open PR** from your fork/branch
2. **Automated checks** run (CI pipeline) — fix any failures
3. **Maintainer review** — expect constructive feedback within 1-3 business days
4. **Address feedback** — push additional commits (or fixup + force-push for small changes)
5. **Approval and merge** — squash-merge to master, preserving the commit message

### 8. Release Process

Releases are managed by the maintainer. The process:

1. A `release/vX.Y.Z` branch is cut from master
2. `CHANGELOG.md` is updated with the new version
3. `Cargo.toml` version is bumped
4. A review tag `vX.Y.Z` is created
5. CI auto-builds and publishes to crates.io
6. A GitHub Release is created with release notes

Contributors who have merged PRs since the last release are credited in the release notes.

---

## Research Contribution Guidelines

If you're contributing research results (experiments, benchmarks, analysis):

1. **Reproducibility is mandatory.** Every result must be traceable to a specific commit, seed, config, and environment.
2. **Negative results are welcome.** If your experiment disproves a hypothesis, that's a valuable contribution. Document what you expected, what happened, and why the discrepancy matters.
3. **Document limitations.** If your experiment only tested one configuration or one environment, say so. Trust is built by what you admit.
4. **Raw data over summaries.** Check in the raw CSV/JSON. Summaries and visualizations are nice, but the raw data is what allows others to verify and extend your work.
5. **Statistical rigor.** Report effect sizes, confidence intervals, and number of trials, not just point estimates.

---

## Project Structure

```
neuron-wire/
├── src/                  # Crate source
│   ├── components/       # EntityId, Neuron/Synapse components (ECS)
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

- **Discord:** [Join the community](https://discord.gg/neuron-wire) for real-time discussion
- **GitHub Discussions:** [Start a discussion](https://github.com/cianmag/neuron-wire/discussions) for questions and ideas
- **Code of Conduct:** See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

---

## Questions?

Open a [GitHub Discussion](https://github.com/cianmag/neuron-wire/discussions), ask in our [Discord](https://discord.gg/neuron-wire), or email the maintainer at ci@cianmag.com.
