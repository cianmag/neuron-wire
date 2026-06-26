# Lessons Learned: Heuristics That Survived

*June 2026*

---

Every project generates lessons. Most are specific to the project. A few — the ones worth writing down — survive beyond their original context. These are the ones I'd tell my past self before starting another distributed systems project.

## 1. Build the Verification Infrastructure Before the Protocol

I wrote the DHT routing table before I wrote the round-trip fuzz test. The routing table took 3 days. The fuzz test took 30 minutes. But I spent **6 cumulative days** debugging routing bugs that the fuzz test would have caught in 30 seconds.

The pattern is consistent: verification code written late catches fewer bugs because it's shaped by the same assumptions that created the bugs. The apoptosis eviction bug (Experiment 1) and the NWP offset bug (Experiment 2) were both discovered by tests added **after** the symptom was visible at the integration level. A test written concurrently with the code would have caught them at unit-test granularity, saving the integration-level debugging.

**Heuristic:** For every piece of protocol-level code, write these **before** the implementation:
- A round-trip fuzz test (random inputs through serialize → deserialize → assert_eq)
- A property test linking input invariants to output invariants
- A deterministic integration test with seed=42

The seed=42 test is the most valuable: it locks the exact sequence of events. When a future change breaks something, the diff between the old and new output is the bug report.

## 2. Determinism Is Worth More Than Throughput

There's a moment in every distributed systems project where you choose between:
- **Reproducibility:** Seed RNG, single thread, synchronous sweeps, bounded queues
- **Performance:** Thread pools, async I/O, randomized backoff, lock-free data structures

NWP chose reproducibility at every juncture, and that decision saved us more development time than it cost in runtime. The specific wins:

- **Zero-variance benchmarks.** Every trial with the same seed produces identical results. This means a performance regression is visible in the first trial, not after statistical noise is filtered.
- **Bisectable bug reports.** When a test fails, the exact sequence of packets leading to the failure is deterministic. You can `git bisect` to find the commit that changed the packet ordering.
- **No flaky tests.** In 18 months of development, we had exactly 0 CI failures due to timing or nondeterminism. The CI either passes or reveals a real bug.

The cost is real but manageable: at 50 nodes, convergence takes 4.0 seconds instead of a potentially faster stochastic convergence. The trade-off is **debugging time vs runtime**. In a research project where the runtime is measured in seconds and debugging time in hours, the choice is obvious.

## 3. Use a Single Thread Until You Can't

The conventional scaling playbook says: "Start with async I/O, then optimize." The reality is different. Async introduces complexity from day one. Every function is either `async fn` or can't call one that is. Stack traces from 50 concurrent tasks are unreadable. Backpressure requires explicit channel sizing.

The single-threaded engine loop (Phase 1-6, repeat) is a **game loop** pattern. Game engines have been doing this since the 1980s. The constraints are identical:
- Predictable frame budget (1ms per tick)
- Bounded work per frame (~100 packets at n=50)
- Priority: latency over throughput (sub-millisecond per tick)
- Concurrent I/O handled by non-blocking system calls (recv_from with timeout)

**When to reach for threads:**
1. When a single tick exceeds the budget (1ms) — measure, don't guess
2. When work is embarrassingly parallel and latency-insensitive (e.g., batch neural computation)
3. When you need to block on something with no timeout (disk I/O)

For NWP at n=50, each tick takes ~60μs. The CPU is 94% idle. Adding threads would add synchronization overhead with zero throughput benefit.

**Heuristic:** Profile the idle percentage. If it's above 50%, you don't need threads yet. If it's below 10% and you need more throughput, threads are a last resort — first optimize the hot path.

## 4. The Right Default for Failures Is "Ignore"

When the engine receives a malformed packet, what should it do? Options:
1. **Crash** — the packet is evidence of a bug, fail fast
2. **Log and continue** — record the event for debugging
3. **Silently drop** — unreliable channel, loss is expected

We started with option 1 (crash on any unexpected input) because that's the engineering convention: fail fast, surface bugs immediately, never swallow errors.

This was wrong.

In a DHT running on UDP, **most packets are malformed**. Not because the protocol has bugs (though it did), but because:
- UDP delivers partial datagrams
- The socket receives traffic from non-NWP applications on the same port
- The adversary test framework intentionally corrupts packets
- Network devices introduce bit flips (rare but real)

Every crash from a malformed packet was a false alarm. The actual bug was the crash, not the packet. We switched to "log and drop" and immediately saw two improvements:
1. The engine survived everything the adversary threw at it
2. The corrupted packets became a signal: "your adversary is running at X% corruption rate"

**Heuristic:** In a system where invalid input is part of normal operation (UDP networking, public APIs, user input), "ignore and log" is the only correct default. Reserve "crash" for invariants that cannot be violated even by malicious input — like a violated `assert` in the routing table's binary tree structure, not a wrong magic byte.

The exception: **test mode**. When `--paper-mode` is set, the simulator should crash on any unexpected event because we want to surface every protocol ambiguity. The same code path, different configuration.

## 5. Benchmarks Should Be Self-Verifying

A benchmark that prints "0/1 converged, 3.00s ± 0.00s" and exits 0 is lying. The exit code says "success" but the data says "convergence failed."

Our benchmarks now exit with a **semantic exit code**:
- 0: All trials converged, all assertions passed
- 1: Some trials failed to converge
- 2: Mean convergence time exceeded threshold
- 3: Non-deterministic result (variance > 0 despite fixed seed)
- 4: Zero convergence

This means our CI can block on regression: a PR that degrades convergence time from 3.0s to 4.0s gets exit code 2, not 0. The developer knows immediately that their change regressed performance.

**Heuristic:** A benchmark is not a benchmark if its output requires manual inspection. Every number should have an associated assertion, and every assertion should have an exit code that automation can check.

## 6. The 80/20 of Distributed Systems Debugging

In debugging roughly 40 distinct bugs during NWP development, the distribution of root causes was:

| Category | Count | Signal |
|----------|-------|--------|
| Wrong default value | 11 | Initialize a field with a reasonable-sounding default that beats real values |
| Off-by-one in serialization | 9 | Manual byte offsets, alignment padding, endianness |
| Missing state transition | 7 | DHT handler didn't handle a message type → silent drop |
| Race condition (in supposedly single-threaded code) | 5 | Shared Arc<AtomicBool> updated by simulator, read by engine |
| Protocol misunderstanding | 4 | "Surely it works like TCP" |
| Genuine algorithmic error | 3 | Wrong k-bucket splitting condition |
| Everything else | 1 | Kernel bug (fixed in later kernel version) |

The striking distribution: **wrong defaults and serialization errors account for 50% of bugs**. These are mechanical errors, not deep algorithmic mistakes. They're the easiest to catch with property-based testing and the hardest to find with manual inspection.

**Lesson:** If your distributed system is buggy, the problem is probably not the consensus algorithm. It's probably a `Default` impl or a byte offset. Write the round-trip test first.

---

*Next: [Benchmarks — 50 nodes in 4 seconds](/04-benchmarks.md)*
