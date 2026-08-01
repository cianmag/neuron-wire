# Founder Statement — Neuron Wire Protocol (NWP)

**Author:** [Founder name] · Founder, [Zylvon](https://zylvon.com) · **Date:** 2026-07-31
**Project:** Neuron Wire Protocol — open infrastructure for decentralized AI
**Repository:** [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)

---

## Why I started this

I started Neuron Wire because decentralized AI should not require owning infrastructure. Today, collaborative learning is locked behind parameter servers, orchestrators, and cloud accounts; a developer in a region with unreliable cloud access cannot participate in building the models that shape their world. I wanted to test a different question: can collaborative learning work on a network where any device that can reach another device can join, and no single operator is in control? I began with one laptop, no funding, no team, and no institutional backing — and set out to answer it.

## How I built it with limited infrastructure

Building infrastructure without infrastructure meant making different choices:

- **One machine instead of a fleet.** The engine is a single-threaded, non-blocking loop that sustains roughly 400 kHz–1 MHz ticks on commodity hardware and uses 0% CPU when idle — designed to run on the kind of machine most people actually have.
- **Free CI instead of a lab.** Four GitHub Actions workflows build, test, lint (zero-warning clippy policy), and validate reproducibility on every push.
- **Deterministic simulation instead of real servers.** A paper-mode simulator runs real node objects with real in-process message passing, so experiments are reproducible to the bit — including adversarial scenarios (Byzantine, Sybil, churn, partitions) that would be expensive or impossible to stage for real.

## What I achieved without institutional support

The MIT-licensed repository now contains:

- **20,900+ lines of Rust** across the protocol, engine, security layer, simulator, and benchmarks;
- **342 tests passing** on Linux CI, plus 19 benchmarks and 4 fuzz targets;
- **Simulated convergence of 100,000 nodes** (100% in 7.0 simulated seconds), with the caveat — documented in the repo — that simulation is not reality;
- **Formal specifications**: a 1,760-line formal model (convergence, trust, partition tolerance, entropy bounds) and a 485-line wire-protocol spec with complexity analysis;
- **A security layer**: Ed25519 authentication on every packet, optional XChaCha20-Poly1305 encryption with an experimental X25519 key-exchange path, trust-based Sybil resistance, and a hash-chained audit log — 47 of 61 security-checklist items implemented, with an internal audit published alongside the remaining gaps;
- **Honest negative results**, including a sparse-gradient approach documented as a failure — because a limitation documented is stronger than an assumption hidden.

## What funding would unlock

Three deliverables, each with a pre-committed success metric:

1. A **geographically distributed testbed** — the funded pilot is **20 nodes across three regions (NA/EU/APAC)**, sustained for seven days, with a public metrics dashboard: the step that simulation cannot take. Scaling beyond the pilot is driven by its data.
2. An **external security audit** of the cryptographic and transport layers.
3. **Publication** — reproducible datasets, baselines, and a peer-reviewed paper including negative findings.

## Why I can execute the next stage

I have already demonstrated the behavior funding would buy: working artifacts, not slideware; one-command reproduction of every experiment; documentation that states what is not yet true. The request is not for a seed to start — it is for the boundary crossing from prototype to real-world research infrastructure, and the evidence that the prototype works is in the repository for anyone to verify.

I am 16 years old, a Nepali citizen studying in India. I taught myself Rust and distributed systems without formal instruction, and I have learned to plan around constraints — school schedules, a budget of zero, and a timezone away from most of the open-source world. Those constraints are why this project is designed to be judged by evidence rather than reputation. Give me a testbed, an auditor, and a reviewer, and I will deliver the data.

---

*Supporting documents: [GRANT.md](../GRANT.md) · [GRANT_RISKS.md](GRANT_RISKS.md) · [AUDIT_REPORT.md](AUDIT_REPORT.md) · [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) · [STATS.md](../STATS.md) · [RESEARCH_BRIEF.md](RESEARCH_BRIEF.md)*
