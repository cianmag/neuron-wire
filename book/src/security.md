# Security Model

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| Impersonation | Ed25519 signatures on every packet |
| Eavesdropping | XChaCha20-Poly1305 AEAD encryption |
| Replay attacks | Monotonic nonce counter + replay detection |
| Sybil attacks | Trust-based rate limiting |
| DoS/flood | Per-peer + global rate limiting, connection limits |
| Tampering | CRC32 header integrity, hash-chain audit log |

## Security Layers

```text
┌─────────────────────────────┐
│  Layer 5: Audit Log         │  Hash-chain tamper detection
├─────────────────────────────┤
│  Layer 4: Trust System      │  Sybil resistance, reputation
├─────────────────────────────┤
│  Layer 3: Rate Limiting     │  Per-peer + global burst limits
├─────────────────────────────┤
│  Layer 2: Encryption        │  XChaCha20-Poly1305 AEAD
├─────────────────────────────┤
│  Layer 1: Authentication    │  Ed25519 signatures
└─────────────────────────────┘
```

## Trust Scoring

Every peer has a trust score in [0.0, 1.0]:

| Event | Delta |
|-------|-------|
| Valid signature | +0.05 |
| Successful handshake | +0.10 |
| Replay attack | -0.80 |
| Invalid signature | -0.50 |
| Packet timeout | -0.10 |

Trust decays at 0.001/second. Scores converge to 0 for inactive peers.

## Rate Limiting

| Trust Level | Burst Limit |
|-------------|-------------|
| Sybil (< 0.2) | 5 packets/sec |
| Normal (0.2-0.7) | 10 packets/sec |
| Trusted (> 0.7) | 100 packets/sec |

Global rate limit applies across all peers.
