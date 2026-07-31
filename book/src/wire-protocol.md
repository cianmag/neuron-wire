# Wire Protocol

## Frame Layout

Every NWP message is wrapped in a frame:

```text
[4-byte frame_len][16-byte MessageHeader][body bytes]
```

## MessageHeader (16 bytes)

```text
[0-3]   magic: [u8; 4]    = "NWP\0"
[4]     version: u8       = 2
[5]     msg_type: u8      = message type discriminant
[6-7]   flags: u16        = bit flags (little-endian)
[8-11]  body_len: u32     = body length (little-endian)
[12-15] header_crc: u32   = CRC32 of bytes [0..12)
```

## Message Types

| Type | ID | Purpose |
|------|-----|---------|
| PING | 7 | DHT liveness probe |
| PONG | 8 | DHT liveness reply |
| FIND_NODE | 9 | DHT lookup query |
| NODES | 10 | DHT lookup response |
| GRADIENT | 20 | Neural gradient data |
| GRADIENT_ACK | 21 | Gradient acknowledgment |
| HEARTBEAT | 30 | Keepalive (empty body) |
| DISCONNECT | 40 | Graceful disconnect |

## Security Flags

| Flag | Bit | Meaning |
|------|-----|---------|
| ENCRYPTED | 0x0001 | AEAD-encrypted payload |
| AUTHENTICATED | 0x0002 | 32B pubkey + 64B signature prefix |
| HANDSHAKE | 0x0004 | Secure channel handshake |
| AUDIT_REQUEST | 0x0008 | Request audit proof |
| BOOTSTRAP | 0x0010 | Bootstrap proof payload |

## Authenticated Frame

```text
[32B Ed25519 public key][64B Ed25519 signature][original body]
```

The signature covers: `seq || timestamp || SHA-256(body)`.
