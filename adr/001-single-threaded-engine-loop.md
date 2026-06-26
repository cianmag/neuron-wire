# ADR 1: Single-Threaded Engine Loop (No Async Runtime)

**Status:** Accepted  
**Date:** 2026-04 (initial), reaffirmed 2026-06  
**Deciders:** Project author

---

## Context

Most modern Rust network services use an async runtime — Tokio, smol, or async-std — to multiplex concurrent I/O across a thread pool. This is the conventional approach for network services that must handle thousands of simultaneous connections.

The neuron-wire runtime needs to:
- Receive UDP packets from multiple peers
- Process DHT routing lookups
- Execute neural computation (forward pass + STDP)
- Manage neurogenesis and apoptosis
- Schedule periodic maintenance (retransmission, pruning)
- All of this with minimal jitter for deterministic benchmarking

The obvious choice is Tokio with `tokio::select!` or async tasks. Many projects choose this path.

## Decision

Use a **single-threaded, non-blocking engine loop** without any async runtime. The loop structure is:

```
loop {
    1. Drain UDP recv queue (non-blocking recv_from until WouldBlock)
    2. Drain outbound channel (send queued packets)
    3. Neural compute (forward pass + Hebbian STDP)
    4. Retransmit (scan reliable queue)
    5. Apoptosis (prune dead state)
    6. Yield / block on recv_from()
}
```

Key properties:
- No Tokio, no async, no work-stealing scheduler
- Single `std::net::UdpSocket` with blocking `recv_from()` and 1ms timeout
- All phases execute in deterministic order within a single tick
- No `Send + Sync` constraints — shared state is just `&mut self`
- Total transitive dependencies: ~25 crates vs. 50+ for an equivalent Tokio-based runtime

## Consequences

**Positive:**
- Eliminates scheduler jitter — deterministic execution across runs
- No Send/Sync complexity — the compiler doesn't fight ownership across task boundaries
- Faster cold start — no async runtime initialization, no thread pool warmup
- Smaller dependency tree — avoids the Tokio ecosystem's 50+ transitive crates
- Easier to reason about — linear execution, no implicit yield points
- Better cache behavior — hot code and data stay on one core
- Zero-overhead inter-phase communication — phases share `&mut EngineState` directly

**Negative:**
- Cannot exploit multi-core for the engine loop (but subsystems could be offloaded)
- One slow phase blocks all others (though in practice phases are O(n) scan operations)
- Not idiomatic modern Rust — most OSS network services use Tokio
- Blocking `recv_from()` with timeout is less efficient than epoll-driven I/O under very high concurrent socket counts (not relevant for a single socket)
- At scale (>10^4 nodes), per-tick processing may exceed the target 1ms budget

**Trade-offs accepted:**
- The engine tick rate target is 1 KHz–1 MHz (1ms–1μs per tick). Profiling shows the current implementation sustains this comfortably on 512MB RAM VPS hardware
- Multi-core scaling is deferred: if the runtime ever exceeds a single core's capacity, a sharded design (one engine per core, shard-local routing tables) would replace the single-threaded approach

## Related

- This decision enables ADR-7 (Deterministic Simulation / Paper Mode) — jitter-free execution is a prerequisite for reproducible benchmarks
- The engine loop structure is documented in detail in `ARCHITECTURE.md` § "The 6-Phase Engine Loop"
- Implementation: `src/engine_loop.rs`

## References

- "Your Server as a Function" — the single-threaded event loop pattern (Node.js, nginx, Redis)
- Tokio vs. manual epoll benchmarks: single-threaded event loops match or exceed async runtime throughput for I/O-bound workloads below 10K concurrent connections
- Redis (~100K ops/sec on a single core) demonstrates the viability of single-threaded event-driven architectures
