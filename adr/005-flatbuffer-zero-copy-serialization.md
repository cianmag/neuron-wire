# ADR 5: FlatBuffer Zero-Copy Serialization

**Status:** Accepted  
**Date:** 2026-04 (initial)  
**Deciders:** Project author

---

## Context

Every message sent between nodes must be serialized and deserialized. For a runtime targeting ~1 MHz tick rates, serialization overhead directly limits throughput.

Options considered:
- **JSON** — human-readable, ubiquitous, but ~10–50x slower than binary formats for large messages
- **Protobuf** — well-supported, compact binary encoding, but requires a separate parsing step for field access
- **MessagePack** — compact, fast, but no schema validation
- **Cap'n Proto** — zero-copy design similar to FlatBuffers, less mature ecosystem
- **FlatBuffers** — zero-copy wire format: no parsing step, fields accessed directly from the buffer
- **Raw binary** — maximally efficient but error-prone, no schema enforcement

The critical requirement is that the engine loop (ADR-1) must not block on deserialization. In a zero-copy format, receiving a message and reading its fields are the same operation — there's no parse step.

## Decision

Use **Google FlatBuffers** for all wire-format serialization.

All NWP messages follow this framing:

```
[16 bytes] Transport Header   (seq, ack, bitfield, timestamp)
[16 bytes] NWP Message Header (magic, version, type, flags, body_len, CRC32)
[N bytes]  FlatBuffer Body    (zero-copy field access)
```

Key design choices:
- The transport header is a fixed-size `#[repr(C)]` struct, not a FlatBuffer — avoids FlatBuffer overhead for the most-frequently-accessed fields
- The NWP header is also fixed-size, with a CRC32 of the body for corruption detection
- The body is a FlatBuffer containing the message-type-specific fields
- On receive: the `recv_from()` buffer is shared directly — no copy, no parse. Field access is `body->activation()`, `body->weight()`, etc.
- On send: FlatBuffers are built into a `Vec<u8>` and queued in the outbound channel

Message types encoded as FlatBuffer unions:
- `Ping` / `Pong` — DHT maintenance (no body beyond header)
- `Command` — control messages (config, shutdown, etc.)
- `Spike` — neural activation event (target neuron ID, activation value)
- `Readiness` — gradient readiness notification
- `Data` — gradient exchange (synapse weights, activation deltas)
- `Consensus` — consensus votes

## Consequences

**Positive:**
- Zero-copy reads: receiving 2,000 gradient messages in one tick costs nothing extra
- Random field access without parsing: a message with 1,000 synapses can be read at any `synapse(i)` without scanning preceding fields
- Schema-enforced wire format: incompatible nodes fail at the protocol level, not with deserialization errors
- Compact wire representation (~40 bytes overhead per message + FlatBuffer body)
- Generated code compiles with zero warnings = confidence in wire format correctness
- Forward compatibility: FlatBuffer schemas evolve without breaking existing nodes

**Negative:**
- Binary wire format is opaque — debugging requires a schema-aware tool or explicit hex dumps (noted in limitations)
- FlatBuffers are a Google dependency — adds one more external crate (though the codebase stays at 5 total dependencies)
- Write path requires a FlatBufferBuilder (heap allocation per message) — not zero-copy on the send side
- Learning curve for contributors unfamiliar with the FlatBuffers API
- No streaming support — each message must be a complete FlatBuffer (acceptable for message-based protocol)

**Measured impact:**
- Zero-copy roundtrip tests pass (unit tests in `src/zerocopy.rs`, `src/flat.rs`)
- Transport header size: 16 bytes, NWP header size: 16 bytes — total overhead: 32 bytes + FlatBuffer body
- No microbenchmarks yet comparing FlatBuffer vs. raw binary for this workload (on the performance profiling roadmap)

## Related

- ADR-1: Engine loop — zero-copy is essential for sustaining the 1 MHz tick rate target
- ADR-2: UDP transport — the wire format framing is designed around UDP datagram boundaries

## References

- Google FlatBuffers. (2014). https://flatbuffers.dev
- FlatBuffers vs. Protobuf benchmarks: FlatBuffers ~0ns decode (no parsing), Protobuf ~100–500ns decode for comparable messages
- Cap'n Proto — alternative zero-copy format, rejected for smaller Rust ecosystem at time of decision
- Implementation: `src/flat.rs`, `src/zerocopy.rs`
