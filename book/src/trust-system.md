# Trust System

## Overview

The trust system provides Sybil resistance through reputation tracking. Every peer starts at 0.5 and evolves based on behavior.

## Convergence Properties

**Theorem 1**: For any peer with score s > 0 and no further interactions, s → 0 as t → ∞ with rate O(λt) where λ = 0.001/sec.

**Theorem 2**: A peer can recover from SYBIL_THRESHOLD (0.2) to TRUSTED_THRESHOLD (0.7) through 5+ consecutive successful handshakes.

## Sybil Resistance

New peers are rate-limited to 5 packets/sec. Creating N Sybil identities produces N×R global traffic, caught by the global rate limiter regardless of per-peer limits.

## Persistence

Trust scores are serialized to binary format:
```text
[u32 count][per peer: 32B EntityId + f32 score + u64 total_events]
```

Transient rate-limit state resets on restart (correct: rate limits should not persist).
