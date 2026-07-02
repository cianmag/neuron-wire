# Paper 5: NWP-Bench — A Reproducible Benchmark Suite for P2P Neural Computation

**Target venue:** Bench / MLPerf / arXiv
**Status:** Draft
**Estimated pages:** 8–10

---

## Abstract

Benchmarking decentralized learning systems is notoriously difficult because results depend on network topology, timing, failure injection, and random seeds. We present NWP-Bench, a reproducible benchmark suite for P2P neural computation runtimes. NWP-Bench consists of 10 standardized experiments covering convergence, scaling, bandwidth, churn tolerance, failure recovery, and statistical stability. Every experiment uses `--paper-mode` with fixed seeds, frozen configuration snapshots (`experiment.toml`), and output CSVs that are automatically validated against known-good reference data. The benchmark suite runs from a single command (`bash scripts/reproduce.sh`), captures full environment metadata (compiler, OS, CPU, dependency tree, source hashes), and generates publication-ready figures. NWP-Bench has been used to produce all results in the neuron-wire paper series and is designed for easy extension to other P2P learning systems.

## Key Claims

1. **Fully reproducible** — same seed, same commit, same config, same CSVs
2. **Comprehensive** — 10 experiments covering 6 dimensions of system performance
3. **Self-validating** — CI enforces match against known-good reference data
4. **Portable** — pure Python+Rust, no external services, runs on any Linux host

## Outline

1. Introduction
2. Benchmark Methodology
3. Experiment Suite
4. Environment Capture
5. Validation
6. Figure Generation
7. Case Study: Results
8. Extending NWP-Bench
9. Conclusion

## Data Needed

- [x] Experiment configuration files (10 figures)
- [x] Validation scripts
- [x] Figure generation scripts
- [ ] Cross-platform validation results
