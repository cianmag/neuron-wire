# ADR 2: UDP Transport with Reliability Tiers (Not TCP)

**Status:** Accepted  
**Date:** 2026-04 (initial)  
**Deciders:** Project author

---

## Context

Choosing a transport protocol is one of the most consequential decisions for a distributed runtime. The transport layer must handle:

- **Neural activation messages** (SPIKE) — frequent, small, latency-sensitive, loss-tolerant
- **Gradient exchange** (DATA) — larger payloads, requires eventual delivery, can tolerate some loss
- **Control messages** (COMMAND, CONSENSUS) — rare but must be delivered reliably
- **DHT pings** (PING/PONG) — periodic keepalives, can be lost without harm

TCP provides built-in reliability, in-order delivery, and congestion control. It is the default choice for most network protocols.

**The problem with TCP for this workload:** Neural computation requires low-latency message delivery. TCP's head-of-line blocking means a single lost packet stalls the entire connection queue until retransmission completes. In a peer-to-peer network with hundreds of concurrent peer connections, maintaining TCP connections to all peers would require either one socket per peer (O(N) file descriptors) or connection multiplexing.

UDP provides none of TCP's guarantees but allows message-level granularity, zero connection setup overhead, and natural integration with the single-threaded event loop.

## Decision

Use **UDP as the sole transport protocol**, with a custom reliability layer layered on top.

The reliability layer provides three tiers:

| Tier | Max Retries | Backoff | Used For | Delivery Guarantee |
|------|-------------|---------|----------|--------------------|
| BestEffort | 0 | — | SPIKE, COMMAND, PING, GOSSIP | None (fire-and-forget) |
| Data | 3 | 10ms, 100ms, 1000ms | Gradient DATA frames | At-least-once under low loss |
| Consensus | 5 | Exponential (10ms, 50ms, 250ms, 1s, 5s) | CONSENSUS frames | At-least-once under moderate loss |

The reliability layer is implemented as a `ReliableQueue` in `src/transport.rs`:

```
struct ReliableQueueEntry {
    seq: u32,                // monotonic per-sender sequence number
    data: Vec<u8>,           // serialized FlatBuffer
    retries_remaining: u8,   // max 3 for Data, 5 for Consensus
    next_retry_at: Tick,     // tick when retransmission fires
    weight: f32,             // gradient weight (decays with age)
}
```

ACK/NACK is handled via a sliding window bitfield in the transport header:

```
[ 0- 3] sequence_number  (last sent seq)
[ 4- 7] ack_number       (last contiguous seq received)
[ 8-11] ack_bitfield     (bitmask of next 32 packets)
[12-15] timestamp        (sender's local tick in ms)
```

This means a single UDP datagram both carries data and acknowledges up to 33 previous packets.

## Consequences

**Positive:**
- No head-of-line blocking — an expensive gradient retransmission doesn't stall SPIKE messages to the same peer
- No connection state per peer — a single UDP socket handles all peers
- Natural fit with the single-threaded event loop — `recv_from()` returns both data and source address, no accept/connection map needed
- Gradient weight decay is naturally integrated: stale unacknowledged gradients automatically lose weight and get dropped
- Clock synchronization not required — timestamps are relative to the sender's local tick

**Negative:**
- Reliability is best-effort — under high packet loss (>10%), Data-tier messages may be lost
- Custom reliability layer must be tested and debugged (younger code than battle-tested TCP stacks)
- NAT traversal is harder — UDP hole-punching is less reliable than TCP (but both have issues)
- No built-in congestion control — the runtime must manage its own sending rate
- Large messages (> ~1400 bytes MTU) need application-layer fragmentation (not implemented yet)

**Key trade-off accepted:**
The lack of guaranteed delivery is acceptable because the learning algorithm (Hebbian STDP) is inherently noise-tolerant. Missing a gradient update is equivalent to a slightly smaller mini-batch, not a correctness failure. The gradient weight decay function naturally prioritizes fresh information over retransmitted stale data.

## Related

- ADR-1: Single-threaded engine loop — UDP's stateless nature simplifies the non-blocking recv loop
- ADR-5: FlatBuffer serialization — the wire format piggybacks on FlatBuffer for zero-copy body parsing

## References

- RFC 768 — User Datagram Protocol
- Kademlia (Maymounkov & Mazières, 2002) — also uses UDP with application-layer timeouts
- QUIC (RFC 9000) — the industry trend is toward UDP + application-layer reliability; this project follows the same pattern at a simpler level
- Gradient noise tolerance in decentralized SGD — missing gradients degrade convergence rate but not correctness (Stich, 2019)
