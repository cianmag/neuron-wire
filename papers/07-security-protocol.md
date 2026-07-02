# Paper 7: The NWP Security Protocol — Authenticated, Encrypted, Byzantine-Resistant P2P Communication

**Target venue:** IEEE S&P / CCS / NDSS workshop
**Status:** Planning
**Estimated pages:** 10–12

---

## Abstract

Neuron-wire's current wire format transmits all messages in plaintext with no authentication. A malicious peer can read, inject, modify, or drop frames at will. We specify and implement the NWP Security Protocol, a transport-layer security layer built on the Noise Protocol Framework (NK pattern) with ed25519 identity keys, ChaCha20-Poly1305 authenticated encryption, and replay protection via monotonic sequence numbers. We extend the DHT to support signed peer announcements, preventing Sybil attacks through proof-of-work for node ID registration. We specify a lightweight Byzantine fault tolerance mechanism for consensus messages using threshold signatures. We evaluate the performance overhead: encrypted frame size is 48 bytes larger than plaintext, encryption adds ~2 μs per frame on commodity hardware, and the DHT proof-of-work adds ~100 ms to node bootstrap.

## Key Claims

1. **Authenticated encryption** — every frame is signed by the sender's ed25519 key
2. **Sybil resistance** — proof-of-work for Node ID registration in DHT
3. **Lightweight BFT** — threshold signatures for consensus messages
4. **Practical overhead** — <50 bytes per frame, ~2 μs encryption time

## Outline

1. Introduction
2. Threat Model
3. Noise Protocol Integration
4. DHT Security
5. Byzantine Consensus
6. Performance Evaluation
7. Security Analysis
8. Related Work
9. Conclusion

## Prerequisites

- [x] ed25519-dalek dependency (already in Cargo.toml)
- [x] ChaCha20-Poly1305 dependency (already in Cargo.toml)
- [x] Blake2 hashing (already in Cargo.toml)
- [ ] Wire-level encryption in transport layer
- [ ] Signed DHT announcements
- [ ] Proof-of-work for node IDs
- [ ] Threshold signature consensus
