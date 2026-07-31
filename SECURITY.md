# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| v0.3.0  | ✅ (current release) |
| < v0.3  | ❌ (pre-release prototypes) |

## Reporting a Vulnerability

If you discover a vulnerability that affects the safety of anyone deploying or experimenting with this software:

1. **Do not** open a public GitHub issue.
2. Send a description to the maintainer at **team@zylvon.com** (or open a [GitHub Security Advisory](https://github.com/cianmag/neuron-wire/security/advisories)).
3. Include the word "neuron-wire" in the subject line.
4. Provide detailed steps to reproduce the issue.

You should receive a response within 72 hours. If you don't, follow up.

## What to Expect

- **Critical vulnerabilities** (remote code execution, data loss) will be prioritized and patched within 2 weeks.
- **Moderate issues** (partial information disclosure, denial of service) will be addressed on the regular development timeline.
- **Low-severity issues** (theoretical attacks, missing hardening) will be documented as known limitations and scheduled per the roadmap.

## Current Security Posture

### Implemented (v0.3.0)

| Property | Status | Detail |
|----------|--------|--------|
| **Identity** | ✅ | Ed25519 keypairs per node (OsRng). `EntityId = SHA-256(public_key)`. Key rotation with certificate chain support. |
| **Packet Authentication** | ✅ | Every outbound packet is Ed25519-signed (32B public key + 64B signature prefix). Inbound signatures verified before processing. Invalid signatures dropped, logged, and reduce sender trust. |
| **Replay Protection** | ✅ | Encrypted packets use monotonic nonce counters (1024-entry ring buffer). Signed-only packets use transport header sequence numbers. |
| **Integrity** | ✅ | Ed25519 signatures provide cryptographic integrity. CRC32 remains for accidental corruption detection. |
| **Encryption** | ✅ | XChaCha20-Poly1305 AEAD per packet. Noise-like XX handshake (simplified: SHA-256 key derivation). Enabled via `encrypt_payloads` config flag. |
| **Rate Limiting** | ✅ | Trust system enforces max N packets per time window per peer. Configurable burst and window size. |
| **Trust Scoring** | ✅ | Peers start at 0.5 trust. Positive events (+0.05 to +0.02) and negative events (-0.1 to -0.8) adjust scores. Decay over inactivity. |
| **Audit Log** | ✅ | Hash-chain audit log (tamper-evident). Records NodeStartup, PeerDiscovered, HandshakeSuccess, InvalidSignature, etc. Bootstrap proofs for verification. |

### Not Yet Implemented

| Property | Status | Priority |
|----------|--------|----------|
| X25519 ECDH handshake | ❌ | High — current handshake uses simplified SHA-256 derivation |
| Persistent key storage | ❌ | Medium — keys are in-memory only; lost on restart |
| STUN/TURN NAT traversal | ❌ | High — required for WAN deployment |
| Certificate authority / PKI | ❌ | Low — self-signed identities for research prototype |
| DoS amplification protection | ❌ | Medium — rate limiting exists but no reflection/amplification guard |
| Formal verification | ❌ | Low — verification of protocol logic |

## Architecture

The security layer has four subsystems, all wired into the engine loop:

```
┌──────────────┐     ┌──────────────────┐     ┌────────────┐     ┌──────────┐
│   identity   │────→│  secure_channel  │────→│   trust    │────→│  audit   │
│ (keypairs,   │     │  (encryption,    │     │ (scoring,  │     │ (hash    │
│  signing,    │     │   handshake,     │     │  sybil,    │     │  chain,  │
│  EntityId)   │     │   replay-prot)   │     │  rate-lmt) │     │  proofs) │
└──────────────┘     └──────────────────┘     └────────────┘     └──────────┘
```

- **Engine loop** calls `seal_outbound()` (sign + optionally encrypt) before every send
- **Engine loop** calls `open_inbound()` (verify + optionally decrypt) after every receive
- Invalid packets are dropped, logged, and reduce the sender's trust score
- Security metrics tracked: `authenticated_packets`, `encrypted_packets`, `auth_failures`, `decrypt_failures`

## Configuration

Security is configured via `EngineConfig`:

```rust
EngineConfig {
    security_enabled: true,   // Enable packet signing + verification
    encrypt_payloads: false,  // Enable AEAD encryption (requires handshake)
    identity_seed: None,      // None = random; Some(seed) = deterministic
    // ...
}
```

## Dependencies

- `ed25519-dalek` v2 — Ed25519 signing and verification
- `chacha20poly1305` v0.10 — AEAD encryption
- `blake2` v0.10 — Hashing (nonce expansion)
- `sha2` v0.10 — SHA-256 for EntityId derivation
- `zeroize` v1 — Secure key erasure
- `rand_core` v0.6 — CSPRNG (OsRng)
