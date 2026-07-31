# Neuron Wire Protocol — Security Hardening Checklist

> Comprehensive security review of all attack surfaces. Each item is marked
> ✅ (implemented), ⚠️ (partial), or ❌ (not yet addressed).

---

## 1. Authentication & Identity

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 1.1 | Ed25519 keypair generation | ✅ | `NodeIdentity::new()` uses OsRng |
| 1.2 | Signature verification on all inbound packets | ✅ | `verify_packet_signature()` in engine_loop |
| 1.3 | Key rotation support | ✅ | `NodeIdentity::rotate()` |
| 1.4 | EntityId derivation from public key | ✅ | SHA-256 hash of pubkey |
| 1.5 | Replay attack detection | ✅ | AEAD nonce reuse detection in SecureChannel |
| 1.6 | Zeroize key material on drop | ❌ | Keys not zeroized from memory |
| 1.7 | Constant-time signature comparison | ⚠️ | Depends on ed25519-dalek internals |

## 2. Encryption

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 2.1 | XChaCha20-Poly1305 AEAD | ✅ | Forward secrecy via ephemeral keys |
| 2.2 | Forward secrecy (ephemeral sessions) | ✅ | New session per connection |
| 2.3 | Nonce uniqueness per message | ✅ | Counter-based nonce generation |
| 2.4 | Associated data (AAD) binding | ✅ | Context bytes bound to ciphertext |
| 2.5 | Decryption failure = silent drop | ✅ | No error leak on bad ciphertext |
| 2.6 | Key exchange authenticated | ✅ | Ed25519 signs the exchange |

## 3. Rate Limiting

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 3.1 | Per-peer rate limiting | ✅ | Trust-weighted burst limits |
| 3.2 | Global rate limiting | ✅ | Cross-peer global cap |
| 3.3 | Sybil threshold detection | ✅ | score < 0.2 = untrusted |
| 3.4 | Rate limit bypass prevention | ✅ | Known-peer bypass for existing sessions |
| 3.5 | Cooldown period after breach | ✅ | Rate-limited until window expires |
| 3.6 | DoS via memory exhaustion | ⚠️ | MAX_TRACKED_PEERS=1000, but no per-peer memory cap |

## 4. Connection Management

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 4.1 | Max peers enforcement | ✅ | TOO_MANY_PEERS disconnect |
| 4.2 | Graceful disconnect protocol | ✅ | TYPE 40 with reason codes |
| 4.3 | Peer TTL eviction | ✅ | 5-minute inactivity TTL |
| 4.4 | Heartbeat keepalive | ✅ | 30s interval |
| 4.5 | Connection limit per IP | ✅ | per_ip_max_peers config (default: 10) |
| 4.6 | SYN flood protection | ⚠️ | Not applicable (UDP), but bootstrap spam possible |

## 5. Input Validation

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 5.1 | Minimum packet size check | ✅ | TransportHeader::SIZE + 4 minimum |
| 5.2 | Maximum packet size check | ⚠️ | 65535 byte buffer, no explicit max check |
| 5.3 | Header field range validation | ✅ | `validate()` checks msg_type, flags |
| 5.4 | Body length consistency | ✅ | frame_len matches actual body |
| 5.5 | Fuzz testing of parsers | ✅ | 4 fuzz targets in fuzz/ |
| 5.6 | Panics in parsing = bugs | ✅ | catch_unwind in fuzz targets |
| 5.7 | Integer overflow in length calculations | ⚠️ | Uses `as` casts, not checked arithmetic |

## 6. Trust System

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 6.1 | Trust score bounded [0.0, 1.0] | ✅ | `.clamp(0.0, 1.0)` |
| 6.2 | Time-based trust decay | ✅ | 0.001/sec decay rate |
| 6.3 | Sybil attack resistance | ✅ | New peers rate-limited |
| 6.4 | Trust persistence (save/load) | ✅ | File-based serialization |
| 6.5 | Trust score convergence proof | ✅ | Documented in ALGORITHM_SPEC.md |
| 6.6 | Peer record TTL cleanup | ✅ | 1-hour inactivity eviction |

## 7. Audit & Logging

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 7.1 | Tamper-evident audit log | ✅ | Hash chain integrity |
| 7.2 | Bootstrap proof generation | ✅ | Cryptographic proof of log state |
| 7.3 | Structured JSON logging | ✅ | log_error!/log_warn!/log_info!/log_debug! |
| 7.4 | No secrets in logs | ⚠️ | Peer addresses logged, no keys (verify) |
| 7.5 | Log rotation | ✅ | Docker log rotation configured |
| 7.6 | Audit log size limit | ⚠️ | No explicit max size |

## 8. Network Security

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 8.1 | UDP source validation | ✅ | Rate limit per source address |
| 8.2 | Packet filter allowlist | ✅ | Optional packet_filter_allowed |
| 8.3 | DHT eclipse attack mitigation | ⚠️ | Kademlia k-bucket routing helps, no formal proof |
| 8.4 | Man-in-the-middle prevention | ✅ | AEAD + authenticated key exchange |
| 8.5 | Traffic analysis resistance | ❌ | No padding or dummy traffic |
| 8.6 | NAT traversal (STUN) | ✅ | stun.rs implementation |

## 9. Memory Safety

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 9.1 | All unsafe blocks annotated | ✅ | 13 blocks, all with `// SAFETY:` comments |
| 9.2 | No use-after-free | ✅ | Rust ownership prevents |
| 9.3 | No buffer overflow | ✅ | Bounds checks on all array access |
| 9.4 | No data races | ✅ | Single-threaded engine, Arc<Mutex> for shared |
| 9.5 | Stack overflow prevention | ⚠️ | Deep recursion possible in neural net |
| 9.6 | OOM protection | ⚠️ | No explicit memory limits in-process |

## 10. Operational Security

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 10.1 | Non-root Docker container | ✅ | USER nwp in Dockerfile |
| 10.2 | Read-only filesystem | ❌ | Not configured |
| 10.3 | Resource limits | ✅ | Docker compose resource limits |
| 10.4 | Health check endpoint | ✅ | /health, /metrics, /status |
| 10.5 | Graceful shutdown | ✅ | Ctrl+C → shutdown signal |
| 10.6 | Trust score persistence on shutdown | ✅ | save_to_file() |

---

## Summary

| Category | ✅ | ⚠️ | ❌ |
|----------|---|---|---|
| Authentication | 5 | 1 | 1 |
| Encryption | 6 | 0 | 0 |
| Rate Limiting | 5 | 1 | 0 |
| Connection Mgmt | 5 | 1 | 0 |
| Input Validation | 4 | 2 | 0 |
| Trust System | 6 | 0 | 0 |
| Audit & Logging | 4 | 2 | 0 |
| Network Security | 4 | 1 | 1 |
| Memory Safety | 4 | 2 | 0 |
| Operational Sec | 4 | 0 | 2 |
| **Total** | **47** | **10** | **4** |

### Priority Fixes (❌ items)

1. **Zeroize key material** — Use `zeroize` crate on `NodeIdentity` drop
2. **Per-IP connection limit** — Prevent single IP from filling all slots
3. **Traffic analysis resistance** — Add padding to fixed-size packets
4. **Read-only Docker filesystem** — Add `read_only: true` to docker-compose
5. **Audit log size limit** — Cap at N entries, rotate to file

### Recommended Next Steps

1. Add `zeroize` crate and `impl Drop for NodeIdentity`
2. Add `per_ip_max_peers` config to EngineConfig
3. Add packet padding module (fixed 512-byte packets)
4. Docker compose: `read_only: true` + tmpfs for writable paths
5. Audit log: max_entries config with file rotation
