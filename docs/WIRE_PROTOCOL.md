# NWP v2 — Wire Protocol Specification

**Version:** 2.0  
**Status:** Production  
**Date:** 2026-07-22  
**Author:** Zylvon / Neuron Wire Project

---

## 1. Overview

NWP (Neuron Wire Protocol) is a UDP-based protocol for P2P peer discovery,
authenticated messaging, and encrypted gradient exchange in decentralized neural
networks. It operates on top of UDP with application-layer reliability, identity,
and encryption.

### 1.1 Design Goals

| Goal | Mechanism |
|------|-----------|
| **Authenticity** | Ed25519 signatures on every packet |
| **Confidentiality** | XChaCha20-Poly1305 AEAD encryption |
| **Forward Secrecy** | Ephemeral X25519 key exchange per session |
| **Integrity** | CRC32 header + AEAD authentication tags |
| **Replay Protection** | Monotonic nonce counters per session |
| **Sybil Resistance** | Trust-system-based rate limiting |
| **NAT Traversal** | STUN auto-discovery + DNS seed bootstrap |

### 1.2 Dependencies

| Component | Algorithm | Library |
|-----------|-----------|---------|
| Signatures | Ed25519 | ed25519-dalek 2.x |
| Key Exchange | X25519 | x25519-dalek 2.x |
| Encryption | XChaCha20-Poly1305 | chacha20poly1305 0.10 |
| Hashing | SHA-256 | sha2 0.10 |
| ID Derivation | SHA-256(public_key) | sha2 0.10 |
| Header Checksum | CRC32 | crc32fast 1.x |

---

## 2. Header Format

Every NWP message begins with a 16-byte header:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Magic ("NWP\0")       |   Version     |   MsgType     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Flags (u16)          |        Body Length (u32)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Header CRC32 (u32)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 2.1 Fields

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0-3 | 4B | `magic` | ASCII `"NWP\0"` (0x4E, 0x57, 0x50, 0x00) |
| 4 | 1B | `version` | Protocol version (currently `2`) |
| 5 | 1B | `msg_type` | Message type discriminator (see §3) |
| 6-7 | 2B | `flags` | Bit flags (see §2.2) |
| 8-11 | 4B | `body_len` | Body length in bytes (little-endian) |
| 12-15 | 4B | `header_crc` | CRC32 of bytes [0..12) (little-endian) |

### 2.2 Flags

| Bit | Name | Value | Description |
|-----|------|-------|-------------|
| 0 | `FLAG_ENCRYPTED` | 0x0001 | Body is AEAD-encrypted |
| 1 | `FLAG_AUTHENTICATED` | 0x0002 | Body prefixed with auth (pubkey + sig) |
| 2 | `FLAG_HANDSHAKE` | 0x0004 | Auth prefix includes ephemeral pubkey |
| 3 | `FLAG_AUDIT_REQUEST` | 0x0008 | Sender requests audit proof |
| 4 | `FLAG_BOOTSTRAP` | 0x0010 | Body is a bootstrap proof payload |

### 2.3 Version Validation

Receivers MUST reject packets where `version != 2` with `HeaderError::BadVersion`.
Receivers MUST reject packets where `magic != "NWP\0"` with `HeaderError::BadMagic`.
Receivers MUST reject packets where CRC32 does not match with `HeaderError::BadCrc`.

---

## 3. Message Types

| Type | Value | Direction | Description |
|------|-------|-----------|-------------|
| Ping | 0 | A→B | DHT probe — are you alive? |
| Pong | 1 | B→A | DHT response — I'm alive |
| FindNode | 2 | A→B | DHT lookup — who knows this ID? |
| Nodes | 3 | B→A | DHT response — here are closest nodes |
| Gradient | 4 | A→B | Neural gradient data |
| GradientAck | 5 | B→A | Acknowledgment of gradient receipt |
| Heartbeat | 6 | A→B | Keepalive (no payload) |
| Disconnect | 7 | A→B | Graceful session teardown |

> **Note:** DHT messages use types 7-10 (`dht::dht_msg_type`). Application
> message types (Gradient=20, GradientAck=21, Heartbeat=30, Disconnect=40)
> are defined in `header::msg_type`. See `header.rs` for the authoritative
> registry.

### 3.1 Heartbeat Message (Type 30)

**Body format:** empty (0 bytes)

Heartbeats are periodic keepalive messages sent to all known peers every
`heartbeat_interval_ticks` (default: 30,000 = 30 seconds). They serve two
purposes:

1. **Connection liveness** — informs peers that this node is still active
2. **Eviction prevention** — updates `last_seen` on the receiver, preventing
   the 5-minute TTL eviction from removing active peers

Heartbeats carry no payload. The sender signs them if `sign_packets` is enabled.
The receiver processes them inline in `handle_ingress()` and returns immediately
(no DHT dispatch).

### 3.2 Disconnect Message (Type 40)

**Body format:** `[1-byte reason code][optional UTF-8 detail]`

| Reason | Value | Meaning |
|--------|-------|---------|
| SHUTDOWN | 0 | Node is shutting down gracefully |
| RESTART | 1 | Node is restarting (will reconnect) |
| TOO_MANY_PEERS | 2 | Connection limits reached |
| VERSION_MISMATCH | 3 | Protocol version incompatible |
| RATE_LIMITED | 4 | Peer exceeded rate limit |
| ADMIN | 5 | Manual/admin disconnect |

**Behavior:**
- Receiver logs the reason, removes sender from peer RTT table
- Sent to ALL known peers during shutdown (via `broadcast_disconnect`)
- Not forwarded or relayed — processed at the engine level

---

## 4. Auth Prefix

When `FLAG_AUTHENTICATED` is set, the body begins with an auth prefix before the
actual payload:

### 4.1 Standard Auth (96 bytes)

```
[0..31]   Ed25519 Public Key (32 bytes)
[32..95]  Ed25519 Signature (64 bytes) — signs body[96..]
```

The signature covers everything AFTER the 96-byte prefix: the actual message body.

### 4.2 Auth with Handshake (128 bytes)

When `FLAG_HANDSHAKE` is also set:

```
[0..31]   Ed25519 Public Key (32 bytes)
[32..95]  Ed25519 Signature (64 bytes) — signs body[128..]
[96..127] Ephemeral X25519 Public Key (32 bytes)
```

### 4.3 Identity Derivation

Entity ID = SHA-256(Ed25519 Public Key) — 32 bytes.

### 4.4 Signature Verification

```
entity_id = SHA-256(pubkey_bytes)
verify_ed25519(pubkey_bytes, inner_body, signature_bytes)
```

If verification fails → drop packet, record `InvalidSignature` trust event,
increment `auth_failures` counter.

---

## 5. Encryption

### 5.1 Session Establishment (Warmup Protocol)

Encryption uses per-peer session keys derived via X25519 ECDH:

```
session_key = SHA-256("nwp-handshake-v1" || ECDH(static_sk, peer_static_pk))
```

**Warmup threshold:** 3 authenticated packets must be exchanged before encryption
activates. This prevents the chicken-and-egg deadlock where side A encrypts a
reply before side B has A's public key.

```
Phase 1: A→B (auth-only)     → B creates session, messages_received=1
Phase 2: B→A (auth-only)     → A creates session, messages_received=1
Phase 3: A→B (auth-only)     → B warmup=3 → ready for encryption
Phase 4: B→A (auth+encrypted) → A decrypts → warmup=3 → ready
Phase 5: A→B (auth+encrypted) → Full bidirectional encryption
```

### 5.2 Forward Secrecy (Ephemeral Key Exchange)

During the warmup phase, each side generates a temporary X25519 keypair and
includes the public key in the auth prefix (via `FLAG_HANDSHAKE`):

```
ephemeral_secret = X25519::random()
ephemeral_public = X25519::public_from(ephemeral_secret)
```

After both sides exchange ephemeral public keys:

```
shared_secret = ECDH(ephemeral_priv, peer_ephemeral_pub)
session_key = SHA-256("nwp-ephemeral-v1" || shared_secret)
```

The old static-derived key is replaced. Ephemeral private keys are **never
persisted** — they exist only in memory and are destroyed on shutdown.

**Security guarantee:** Compromising a node's long-term seed cannot decrypt
past sessions because the ephemeral private keys are gone.

### 5.3 Encryption Format

```
Nonce:   [8-byte counter (little-endian)] [16 bytes of zeros]  = 24 bytes total
Plaintext: auth_prefix + payload (or just payload if unauthenticated)
Ciphertext: XChaCha20-Poly1305(nonce, key, plaintext)
```

The 8-byte counter occupies the first 8 bytes of the 24-byte nonce (little-endian).
The remaining 16 bytes are zero.

### 5.4 Replay Protection

Each session maintains:
- `outbound_counter: u64` — incremented on every encrypt
- `received_nonces: VecDeque<u64>` — ring buffer of last 1024 received nonces
- `max_received_nonce: u64` — highest nonce seen

Received nonces that are ≤ `max_received_nonce` AND present in the ring buffer
are dropped as replays.

---

## 6. DHT Protocol

### 6.1 Routing Table

Kademlia-style routing table with 256 buckets (one per bit of the 256-bit
node ID space). Each bucket holds up to 8 entries.

### 6.2 Message Bodies

#### PING (Type 0)

```
[0..31]   Sender NodeId (32 bytes) — SHA-256(sender_pubkey)
[32..38]  Sender Address (7 bytes) — IPv4(4) + Port(2) + Type(1)
[39..42]  Latency estimate (4 bytes, u32 LE) — 0 on first ping
[43..46]  Sequence number (4 bytes, u32 LE)
```

#### PONG (Type 1)

```
[0..3]    Echoed sequence number (4 bytes, u32 LE)
[4..35]   Sender NodeId (32 bytes)
[36..42]  Sender Address (7 bytes)
[43..46]  Latency estimate (4 bytes, u32 LE) — RTT from ping
```

#### FIND_NODE (Type 2)

```
[0..31]   Target NodeId (32 bytes) — who are we looking for?
[32..35]  Sequence number (4 bytes, u32 LE)
```

#### NODES (Type 3)

```
[0..3]    Echoed sequence number (4 bytes, u32 LE)
[4]       Node count (1 byte, u8)
[5..]     Repeated node entries:
          [0..31]   NodeId (32 bytes)
          [32..38]  Address (7 bytes)
          [39..42]  Latency (4 bytes, u32 LE)
```

### 6.3 Iterative Lookup

1. Pick random 32-byte target ID
2. Find K=8 closest known nodes to target
3. Send FIND_NODE(target) to closest node
4. Response returns up to 8 closest nodes
5. Insert all returned nodes into routing table
6. Repeat until no new nodes are discovered

Runs every ~10 seconds (10,000 ticks at 1ms tick rate).

### 6.4 Peer Persistence

Routing table is persisted to binary file:
```
For each node:
  [0..31]   NodeId (32 bytes)
  [32..38]  Address (7 bytes: IPv4 + port + type)
  [39..46]  Latency (8 bytes: f64 LE) — included for reference
```

Loaded on bootstrap, saved every maintenance cycle + on graceful shutdown.

---

## 7. Trust System

### 7.1 Trust Score

Each peer has a trust score ∈ [0.0, 1.0]:

| Event | Delta |
|-------|-------|
| ValidSignature | +0.05 |
| SuccessfulDecrypt | +0.02 |
| SuccessfulHandshake | +0.10 |
| InvalidSignature | -0.50 |
| ReplayAttack | -0.80 |
| PacketTimeout | -0.10 |
| RateLimitExceeded | -0.05 |
| FailedHandshake | -0.20 |

### 7.2 Thresholds

| Threshold | Value | Meaning |
|-----------|-------|---------|
| `SYBIL_THRESHOLD` | 0.3 | Below = untrusted, rate-limited |
| `TRUSTED_THRESHOLD` | 0.7 | Above = fully trusted |

### 7.3 Rate Limiting

Per-peer rate limit: `RATE_LIMIT_BURST = 10` packets per `RATE_LIMIT_WINDOW = 1s`.
Low-trust peers (score < 0.3) are rate-limited more aggressively.
Global rate limit: 10,000 packets/sec across all peers.

### 7.4 Decay

Trust decays by `TRUST_DECAY_PER_SEC = 0.001` per second of inactivity.
Peers inactive for `PEER_TTL = 3600s` (1 hour) are cleaned up.

### 7.5 Persistence

Trust scores saved as binary:
```
[u32 count]
For each peer:
  [0..31]   EntityId (32 bytes)
  [32..35]  Score (f32 LE)
  [36..43]  Total events (u64 LE)
```

Transient rate-limit state is NOT saved — resets on restart (correct behavior:
attackers don't get rate-limit credit across restarts).

---

## 8. Transport Layer

### 8.1 Gradient Weight Decay

```
weight(t) = exp(-λ * t)
λ = ln(2) / half_life_ms
```

Default `half_life_ms = 100.0`.

### 8.2 Reliability

- `BestEffort` — no retries (gradient data)
- `Data` — up to 3 retries, ACK required
- `Consensus` — up to 5 retries, ACK required

### 8.3 NWP v1 Framing

The transport layer wraps application payloads in NWP v1 frames:
```
[4-byte length prefix (u32 LE)]
[16-byte NWP v2 header]
[Body (variable)]
```

---

## 9. Bootstrap Sequence

1. **Load peer cache** (if `peer_cache_path` is set)
2. **Load trust cache** (if `trust_cache_path` is set)
3. **Config bootstrap peers** (from TOML `dht.bootstrap_peers`)
4. **DNS seed resolution** (from `seed_domain` → `_dht.seeds.<domain>:9000`)
5. **Hardcoded seed nodes** (compile-time constants)
6. **STUN probe** (if `stun_enabled`) — discover external IP:port
7. **Passive listen** (if no seeds available)

---

## 10. Graceful Shutdown

1. Set `shutdown` atomic bool (from Ctrl+C handler)
2. Save DHT routing table to `peer_cache_path`
3. Save trust scores to `trust_cache_path`
4. Log final stats (ticks, peers, rx/tx, auth failures, audit events)
5. Ephemeral session keys are **intentionally NOT saved** (forward secrecy)
6. Process exits

---

## 11. Observability Endpoints

| Endpoint | Format | Description |
|----------|--------|-------------|
| `GET /health` | `{"status":"ok"}` | Liveness probe |
| `GET /status` | JSON | Full engine stats |
| `GET /metrics` | Prometheus text | 22 metrics for Grafana |
| `GET /dashboard` | HTML | Live web dashboard |

### 11.1 Prometheus Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `nwp_engine_ticks` | counter | Total ticks executed |
| `nwp_packets_recv` | counter | Total packets received |
| `nwp_packets_sent` | counter | Total packets sent |
| `nwp_bytes_recv` | counter | Total bytes received |
| `nwp_bytes_sent` | counter | Total bytes sent |
| `nwp_retransmissions` | counter | Total retransmissions |
| `nwp_peers` | gauge | Current peer count |
| `nwp_reliable_queue_depth` | gauge | Reliable queue depth |
| `nwp_idle_ratio` | gauge | Fraction of idle ticks |
| `nwp_tick_rate_hz` | gauge | Actual tick rate |
| `nwp_authenticated_packets` | counter | Valid Ed25519 signatures |
| `nwp_encrypted_packets` | counter | AEAD-encrypted packets |
| `nwp_auth_failures` | counter | Signature verification failures |
| `nwp_decrypt_failures` | counter | AEAD decryption failures |
| `nwp_dht_nodes` | gauge | DHT routing table size |
| `nwp_dht_pending_pings` | gauge | DHT pings awaiting PONG |
| `nwp_trust_peers` | gauge | Trust system peer count |
| `nwp_trust_rate_limited` | gauge | Rate-limited peers |
| `nwp_sessions` | gauge | Active secure sessions |
| `nwp_ephemeral_sessions` | gauge | Sessions with forward secrecy |
| `nwp_max_peers` | gauge | Maximum allowed peer connections |
| `nwp_active_peers` | gauge | Current active peer connections |
| `nwp_peer_capacity_ratio` | gauge | Peer capacity utilization (0.0-1.0) |

---

## 12. Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|-----------------|
| Header parse | O(1) | O(1) |
| CRC32 | O(n) | O(1) |
| Ed25519 sign | O(1) | O(1) |
| Ed25519 verify | O(1) | O(1) |
| XChaCha20-Poly1305 encrypt | O(n) | O(n) |
| XChaCha20-Poly1305 decrypt | O(n) | O(n) |
| X25519 ECDH | O(1) | O(1) |
| SHA-256 | O(n) | O(1) |
| DHT bucket index | O(256) = O(1) | O(1) |
| DHT nearest_nodes | O(B*K) | O(K) |
| DHT iterative lookup | O(log(N)*K) | O(N) |
| Trust record_event | O(1) amortized | O(1) |
| Trust check_rate_limit | O(1) amortized | O(1) |
| Trust cleanup_expired | O(N) | O(1) |
| Session encrypt | O(n) + O(1) nonce | O(n) |
| Session decrypt | O(n) + O(1) replay check | O(n) |

Where:
- `n` = message body size (bytes)
- `N` = number of known peers/nodes
- `B` = 256 (bit width of NodeId)
- `K` = 8 (bucket size / nearest_nodes count)
