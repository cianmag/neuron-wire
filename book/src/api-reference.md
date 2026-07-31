# API Reference

## Core Types

### `EngineConfig`
Engine loop configuration. Set `bind_addr`, `max_peers`, `per_ip_max_peers`, `security_enabled`, etc.

### `EngineStats`
Runtime statistics: `packets_sent`, `packets_recv`, `auth_failures`, `active_sessions`, `peer_capacity_ratio`, etc.

### `MessageHeader`
16-byte wire header. Use `build_frame()` to create, `parse_frame()` to read.

### `NodeIdentity`
Ed25519 keypair. Generate with `NodeIdentity::new()`, rotate with `rotate()`.

### `SecureChannel`
Per-peer encrypted sessions. `handshake()` → `encrypt()` → `decrypt()`.

### `TrustSystem`
Trust scoring + rate limiting. `record_event()` → `check_rate_limit()`.

### `AuditLog`
Hash-chain audit trail. `append()` → `verify_integrity()`.

## Key Functions

### `header::build_frame(msg_type, body, flags) -> Vec<u8>`
Build a complete wire frame.

### `header::parse_frame(buf) -> Result<(&Header, &[u8]), HeaderError>`
Parse a wire frame.

### `crc::crc32(data) -> u32`
Hardware-accelerated CRC32 checksum.
