# Research Brief: Neuron Wire (NWP)

**Zero-Infrastructure AI — Open infrastructure for collaborative AI without central servers.**

*1-page summary · Full details at [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)*

---

## Problem

Building distributed AI today requires cloud infrastructure, orchestration, and centralized coordination. Every gradient flows through a parameter server; every participant must trust a central operator. Federated learning still requires a central coordinator. All-Reduce requires a static participant set.

There is no open infrastructure that combines P2P discovery, secure transport, distributed learning, and reproducible experimentation in a single auditable codebase.

## Research Question

*Can collaborative learning work without centralized coordination — where any device that can reach another device can participate, and no single operator controls the network?*

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                Engine Loop (single thread, 6 phases)          │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  DHT Routing  │  │  UDP Socket  │  │  Neural Compute     │  │
│  │  (Kademlia +  │◄─┤  (recv/send) │──┤                    │  │
│  │  latency wt)   │  └──────────────┘  │  ForwardPass →     │  │
│  └──────────────┘                      │  Hebbian STDP →    │  │
│        │                               │  Gossip → Surprise │  │
│        ▼                               └────────────────────┘  │
│  ┌──────────────┐                              │               │
│  │ Birth/Death   │◄──── surprise / prune ──────┘               │
│  │ (Neurogenesis │                                              │
│  │  + Apoptosis) │                                              │
│  └──────────────┘                                              │
└─────────────────────────────────────────────────────────────┘
```

- **Peer discovery**: Latency-weighted Kademlia DHT (256 buckets, K=20)
- **Transport**: Custom UDP with 3 reliability tiers, gradient decay (100 ms half-life)
- **Learning**: Hebbian STDP runs locally; gradients gossip over sparse P2P mesh
- **Runtime**: Single-threaded, non-blocking, ~400 KHz–1 MHz tick rate, 0% CPU when idle
- **No dependencies beyond Rust stdlib** — ~25 transitive crates

## Current Status

| Metric | Value |
|--------|-------|
| Source modules | 42 |
| Lines of Rust | 19,220 |
| Tests | 256 (35/42 modules) |
| CI workflows | 4 |
| Baseline comparisons | 7 (Python) |
| Reproducible experiment configs | 10 |
| Formal mathematical model | 1,760 lines, 17 sections |
| Tutorials | 6 |
| WASM demo | Pure P2P, two browser tabs, no server |
| Build time (from clone) | ~90 seconds |

## What Funding Would Enable

| Deliverable | Description | Success Metric |
|-------------|-------------|---------------|
| D1: Real Internet deployment | 100+ nodes across 3 continents, real metrics | 7-day sustained mesh, public dashboard |
| D2: Publication | Reproducible paper + open datasets | Paper accepted, one-command reproduction |
| D3: Developer SDK | crates.io, PyPI, 5 documented patterns | `cargo add neuron-wire` → running in 5 min |

## Research Discipline

- **Experimental protocol committed before deployment** — timestamped, immutable
- **Negative results published alongside positive ones**
- **Language**: "our experiments suggest," "under the evaluated conditions"
- **Success**: reproducible evidence, not GitHub stars

## Links

- Repository: https://github.com/cianmag/neuron-wire
- Architecture: https://github.com/cianmag/neuron-wire/blob/master/ARCHITECTURE.md
- Formal model: https://github.com/cianmag/neuron-wire/blob/master/FORMAL_MODEL.md
- Grant pitch: https://github.com/cianmag/neuron-wire/blob/master/GRANT.md
- WASM demo: open `demo/www/index.html` in two browser tabs
