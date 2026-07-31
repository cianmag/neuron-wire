# Architecture

## Module Structure

| Module | Purpose | Lines |
|--------|---------|-------|
| `engine_loop` | Single-threaded non-blocking event engine | 1450+ |
| `header` | 16-byte MessageHeader, frame build/parse | 350+ |
| `transport` | UDP send/recv with reliability | 500+ |
| `dht` | Kademlia peer discovery | 600+ |
| `secure_channel` | XChaCha20-Poly1305 sessions | 650+ |
| `trust` | Trust scoring + rate limiting | 550+ |
| `identity` | Ed25519 keypairs + EntityId | 460+ |
| `audit` | Hash-chain tamper detection | 540+ |
| `health` | HTTP health/metrics endpoint | 400+ |
| `flat` | Zero-copy serialization | 300+ |
| `ml` | ML subsystem orchestration | 400+ |
| `hebbian` | STDP learning | 500+ |
| `simulator` | Deterministic network simulation | 800+ |

## Data Flow

### Inbound Packet
```
UDP Socket → recv_from() → parse_frame() → validate() → handle_ingress()
  → rate_limit_check() → decrypt_if_encrypted() → dispatch by msg_type
  → events_tx → subscribers (training, consensus, DHT)
```

### Outbound Packet
```
Component → outbound_tx → engine_loop drain → seal_if_signed()
  → encrypt_if_needed() → send_to() via UDP
```

## Engine Loop Phases

1. **Phase 1: Recv** — drain UDP socket (1ms timeout)
2. **Phase 2: Outbound** — send queued packets
3. **Phase 3: Neural** — forward pass + Hebbian learning
4. **Phase 4: Retransmit** — resend un-ACKed reliable packets (every 10ms)
5. **Phase 5: Cleanup** — peer eviction, apoptosis (every 1s)
6. **Phase 6: Yield** — sleep if no work

## Key Design Decisions

- **Single-threaded** — deterministic timing, zero scheduler jitter, 0% CPU idle
- **No async runtime** — Tokio overhead not needed for recv/send loop
- **Pre-allocated buffers** — no per-packet heap allocation
- **HashMap with capacity** — avoids rehashing during steady state
