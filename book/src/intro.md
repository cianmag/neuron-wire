# Neuron Wire Protocol

**Infrastructure for decentralized AI — P2P peer discovery, gradient exchange, and distributed learning.**

## What is NWP?

Neuron Wire Protocol (NWP) is a zero-copy, single-threaded, non-blocking P2P networking stack written in Rust. It provides:

- **Kademlia DHT** for decentralized peer discovery
- **Ed25519 + XChaCha20-Poly1305** for authenticated, encrypted communication
- **Trust-based rate limiting** with formal convergence guarantees
- **Single-threaded event engine** optimized for free-tier VPS (512MB RAM)
- **Built-in observability** with Prometheus metrics and live dashboard

## Design Principles

1. **Zero external dependencies for core** — stdlib HTTP server only
2. **Single-threaded simplicity** — no async runtime, no work-stealing
3. **Security by default** — every packet authenticated, trust scoring
4. **Observability first** — 28+ Prometheus metrics, live dashboard
5. **Formal correctness** — Big-O analysis, convergence proofs, property-based tests

## Architecture at a Glance

```text
┌──────────────────────────────────────────────┐
│  EngineLoop (single thread, owns UDP socket)  │
│                                                │
│  LOOP (every ~1ms):                            │
│  ├── Phase 1: recv_from() with 1ms timeout     │
│  ├── Phase 2: Drain outbound → send()          │
│  ├── Phase 3: NEURAL COMPUTATION               │
│  ├── Phase 4: Retransmit stale (every 10ms)    │
│  ├── Phase 5: Cleanup + Apoptosis (every 1s)   │
│  └── Phase 6: Yield if busy                    │
└──────────────────────────────────────────────┘
```

## Quick Links

- [GitHub Repository](https://github.com/cianmag/neuron-wire)
- [Architecture Docs](architecture.md)
- [Wire Protocol Spec](wire-protocol.md)
- [Security Model](security.md)
