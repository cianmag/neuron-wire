# ADR 7: Deterministic Simulation / Paper Mode

**Status:** Accepted  
**Date:** 2026-05 (initial)  
**Deciders:** Project author

---

## Context

Reproducibility is the foundation of trust in research. Without it, a result is an anecdote, not evidence.

Most distributed systems projects face a reproducibility problem:
- Network timing is non-deterministic — packet ordering, latency, and loss vary across runs
- Random number generators used for node IDs, weight initialization, and gossip selection introduce variance
- Wall-clock-dependent code (timeouts, periodic tasks) behaves differently on different machines
- Results are reported as summary statistics without raw data

If the project claims to find something surprising (SGA doesn't help routing), that claim must be independently verifiable by anyone with a Rust compiler.

## Decision

Implement a **"paper mode" simulation harness** that guarantees deterministic, reproducible execution.

The system has three layers of determinism:

**1. Seeded RNG:**
```
let mut rng = StdRng::seed_from_u64(config.seed);
// All random operations use &mut rng, not thread-local randomness
// Node IDs, synapse weights, gossip selection — all deterministic
```

**2. Frozen configuration:**
Every experiment captures its entire configuration in `experiment.toml`:
- Node count, seed, duration
- All subsystem parameters (K-bucket size, learning rate, gossip interval, etc.)
- Git commit hash, rustc version, platform metadata

**3. Complete output capture:**
```
results/experiment-001/
├── experiment.toml       # Frozen parameter config
├── metadata.json         # Git commit, rustc, platform, timestamps
├── convergence.csv       # Per-tick gradient variance across all nodes
├── bandwidth.csv         # Aggregate bytes in/out per second
├── routing.csv           # DHT peer count per node over time
├── apoptosis.csv         # Death counts per sweep
├── summary.csv           # One-row aggregation per trial
└── raw/                  # Per-node event logs
    ├── node_000.jsonl
    ├── node_001.jsonl
    └── ...
```

**4. CI validation:**
Every run validates output against known-good CSVs. If the numbers differ, CI fails — catching regressions in the deterministic guarantees.

Usage:
```bash
cargo run --example simulate -- --paper-mode --nodes 10 --duration 120 \
    --output-dir results/experiment-001
```

When `--paper-mode` is set:
- All RNG seeded from `config.seed`
- Timestamps are relative (not absolute wall clock)
- Output is verified against reference CSVs
- Every parameter is frozen in the config file

## Consequences

**Positive:**
- Any experimental result can be verified by anyone with `cargo run` — no cloud credits, no special hardware
- Regression detection: if a code change alters benchmark numbers, CI catches it
- Raw data is checked into the repository — reviewers can analyze it themselves, not just look at summary tables
- The SGA negative result is independently reproducible: any contributor can run the same command and see the same bandwidth numbers
- Builds trust with reviewers and professors

**Negative:**
- Deterministic simulation is not the same as real-world testing — results may not generalize to WAN conditions (documented in the "Limitations" section of every published result)
- Simulation harness adds ~1,000 lines of orchestration code (`simulator.rs`)
- Deterministic RNG means no Monte Carlo variance estimation per trial (separate multi-trial runs are future work)
- Timestamps are relative ticks, not wall time — downstream analysis tools must handle tick-relative time

**Critical limitation (explicitly documented):**
The determinism guarantee applies to the simulator. A multi-node deployment on real hardware would lose determinism due to OS scheduler variation and network timing. The project treats these as separate concerns: simulator results are for hypothesis testing, deployment results are for validation.

## Related

- ADR-1: Single-threaded engine loop — deterministic execution requires jitter-free timing
- ADR-6: Sparse gossip — gossip selection uses seeded RNG for deterministic peer selection
- This ADR enables the benchmark methodology documented in `FOUNDATIONAL_QNA.md` §14 and §16
- Implementation: `src/simulator.rs`, `scripts/repro.sh`

## References

- Peng, R. D. (2011). Reproducible research in computational science. *Science*.
- Sandve, G. K., et al. (2013). Ten simple rules for reproducible computational research. *PLOS Comp. Bio.*
- The practice of checking raw data and configuration into version control (rather than storing it externally) follows the "executable research compendium" model (Gentleman & Lang, 2007).
