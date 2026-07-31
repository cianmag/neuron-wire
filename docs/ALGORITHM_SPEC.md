# Neuron Wire Protocol — Algorithm Specifications

> Formal description of all core algorithms with complexity analysis,
> convergence properties, and security bounds.

---

## 1. Trust Score Algorithm

### 1.1 Definitions

| Symbol | Definition |
|--------|-----------|
| $s_p$ | Trust score for peer $p \in [0, 1]$ |
| $\delta(e)$ | Trust delta for event $e$ |
| $\lambda$ | Decay rate = 0.001 per second |
| $T_{init}$ | Initial trust = 0.5 |
| $T_{sybil}$ | Sybil threshold = 0.2 |
| $T_{trusted}$ | Trusted threshold = 0.7 |

### 1.2 Event Deltas

$$
\delta(e) = \begin{cases}
+0.10 & e = \texttt{SuccessfulHandshake} \\
+0.05 & e = \texttt{ValidSignature} \\
+0.02 & e = \texttt{SuccessfulDecrypt} \\
-0.05 & e = \texttt{RateLimitExceeded} \\
-0.10 & e = \texttt{PacketTimeout} \\
-0.20 & e = \texttt{FailedHandshake} \\
-0.50 & e = \texttt{InvalidSignature} \\
-0.80 & e = \texttt{ReplayAttack}
\end{cases}
$$

### 1.3 Update Rule

When event $e$ occurs for peer $p$ at time $t$:

1. **Apply time-decay**: $s_p \leftarrow \max(0, s_p - \lambda \cdot \Delta t)$ where $\Delta t = t - t_{\text{last}}$ (seconds)
2. **Apply event delta**: $s_p \leftarrow \text{clamp}(s_p + \delta(e), 0, 1)$
3. **Update timestamp**: $t_{\text{last}} \leftarrow t$

### 1.4 Time Complexity

| Operation | Complexity |
|-----------|-----------|
| `record_event()` | $\Theta(1)$ amortized (HashMap insert/lookup) |
| `is_trusted()` | $\Theta(1)$ |
| `cleanup_expired()` | $\Theta(P)$ where $P$ = tracked peers |
| `stats()` | $\Theta(P)$ |

### 1.5 Convergence Theorem

**Theorem 1** (Bounded Trust Scores): For any peer $p$ with score $s_p \in (0, 1]$ and no further interactions, $s_p \to 0$ as $t \to \infty$ with convergence rate $O(\lambda t)$.

*Proof*: In the absence of events, $s_p(t) = \max(0, s_p(t_0) - \lambda(t - t_0))$. Since $\lambda > 0$, the score decreases linearly and reaches 0 in at most $s_p(t_0) / \lambda$ seconds (e.g., 500s for a peer starting at 0.5). $\blacksquare$

**Theorem 2** (Trust Recovery): A peer with score $s_p \geq T_{sybil}$ can recover to $T_{trusted}$ through $n$ consecutive successful handshakes where $n \geq \lceil (T_{trusted} - s_p) / \delta(\texttt{SuccessfulHandshake}) \rceil$.

*Proof*: Each successful handshake adds $+0.10$. From $T_{sybil} = 0.2$ to $T_{trusted} = 0.7$, this requires $\lceil 0.5 / 0.10 \rceil = 5$ consecutive handshakes (ignoring decay). With decay at $\lambda = 0.001/s$, a handshake every 10 seconds would need $5 / (0.10 - 0.01) \approx 56$ handshakes. $\blacksquare$

### 1.6 Sybil Resistance

**Claim**: A Sybil attacker creating $N$ fake identities cannot bypass rate limiting because:
1. Each new peer starts at $T_{init} = 0.5$, rate-limited to $\text{RATE\_LIMIT\_BURST} / 2 = 5$ packets/window
2. Sybil threshold ($T_{sybil} = 0.2$) requires 6+ negative events to reach
3. Global rate limit applies across ALL peers: $\text{global\_count} > \text{global\_rate\_limit}$ → drop

**Corollary**: $N$ Sybil identities each sending $R$ packets/second produce a global rate of $N \cdot R$ packets/second, which is caught by the global rate limiter regardless of per-peer limits.

---

## 2. Rate Limiter Algorithm

### 2.1 Architecture

Two-tier rate limiting: **global** (across all peers) and **per-peer** (trust-weighted).

### 2.2 Global Rate Limiter

A sliding-window counter:

```
if now - global_window_start > 1000ms:
    global_window_start = now
    global_window_count = 0
global_window_count += 1
if global_window_count > global_rate_limit:
    DROP packet
```

**Time complexity**: $\Theta(1)$ per check.
**Space**: $O(1)$ (single counter).

### 2.3 Per-Peer Rate Limiter

Trust-weighted burst limit:

$$
\text{limit}(p) = \begin{cases}
\text{BURST} / 2 = 5 & \text{if } s_p < T_{sybil} \\
\text{BURST} = 10 & \text{if } T_{sybil} \leq s_p < T_{trusted} \\
\text{BURST} \times 10 = 100 & \text{if } s_p \geq T_{trusted}
\end{cases}
$$

**Window**: 1 second ($\text{RATE\_LIMIT\_WINDOW\_MS} = 1000$).
**Cooldown**: After exceeding the burst limit, the peer is rate-limited for a cooldown period.

**Time complexity**: $\Theta(1)$ amortized (HashMap lookup + counter increment).
**Space**: $O(P)$ where $P$ = unique peers tracked (bounded by $\text{MAX\_TRACKED\_PEERS} = 1000$).

### 2.4 Failure Probability

For a well-behaved peer sending $r$ packets/second with burst limit $B$:
- **False positive rate**: 0 (as long as $r \leq B$ packets per window)
- **False negative rate**: 0 (adversary sending $> B$ packets always gets caught)

### 2.5 DoS Resistance

**Theorem 3** (DoS Bound): A single adversary with bandwidth $W$ can force at most $W / (\text{avg\_packet\_size})$ packet drops per second through the global rate limiter. The per-peer limiter ensures each adversary identity is individually throttled.

**Time complexity**: $O(1)$ check per packet → engine throughput is not bottlenecked by rate limiting.

---

## 3. Connection Limit Algorithm

### 3.1 Admission Control

```
if peer_rtt.len() >= config.max_peers:
    if not peer_rtt.contains_key(&src):
        send DISCONNECT(TOO_MANY_PEERS)
        DROP packet
```

**Time complexity**: $\Theta(1)$ amortized (HashMap::len + HashMap::contains_key).
**Space**: $O(P)$ where $P = |\text{peer\_rtt}| \leq \text{max\_peers}$.

### 3.2 Properties

- **Hard limit**: Never exceeds `max_peers` concurrent connections
- **Known-peer bypass**: Existing peers always admitted (stateful tracking)
- **Graceful rejection**: Sends DISCONNECT message with reason code

---

## 4. Peer Eviction Algorithm

### 4.1 TTL-Based Eviction

Runs during cleanup phase (every `cleanup_interval_ticks`):

```rust
let peer_ttl_ms: u64 = 300_000; // 5 minutes
self.peer_rtt.retain(|addr, info| {
    let age = now_ms.saturating_sub(info.last_seen_ms);
    age < peer_ttl_ms
});
```

**Time complexity**: $\Theta(P)$ per sweep (HashMap::retain).
**Amortized**: $O(P / C)$ where $C$ = cleanup interval in ticks.

### 4.2 Trust-Based Eviction

Runs via `trust_system.cleanup_expired()`:

```rust
pub const PEER_TTL_SECS: u64 = 3600; // 1 hour
// Remove peers with no activity for PEER_TTL_SECS
```

**Time complexity**: $\Theta(P)$ per sweep.
**Space**: $O(P)$.

---

## 5. Heartbeat Protocol

### 5.1 Mechanism

- **Type**: Empty HEARTBEAT message (msg_type = 30, no payload)
- **Interval**: Every `heartbeat_interval_ticks` (default: 30,000 ticks ≈ 30 seconds)
- **Flags**: `FLAG_AUTHENTICATED` if security enabled

### 5.2 Bandwidth Overhead

For $P$ peers and interval $I$:
$$\text{overhead} = P \times \frac{H_{\text{size}}}{I} \text{ bytes/second}$$

With $H_{\text{size}} = 20$ bytes (4 frame_len + 16 header), $P = 100$, $I = 30$:
$$\text{overhead} = 100 \times \frac{20}{30} \approx 67 \text{ bytes/second}$$

### 5.3 Failure Detection

A peer that hasn't sent any packet (including heartbeats) for 5 minutes is evicted by the TTL mechanism (Section 4.1). The heartbeat ensures that even idle peers are not evicted.

**Detection bound**: Maximum time to detect a dead peer = $\max(\text{heartbeat\_interval}, \text{eviction\_interval})$ ≈ 5 minutes.

---

## 6. Overall Engine Complexity

### 6.1 Per-Tick Cost

Let $P$ = active peers, $E$ = pending events, $S$ = synapses, $N$ = neurons.

| Phase | Complexity | Frequency |
|-------|-----------|-----------|
| Recv | $O(1)$ amortized | Every tick |
| Ingress processing | $O(P)$ (HashMap ops) | Per packet |
| Neural computation | $O(S + N)$ | Every tick |
| Retransmit scan | $O(P)$ | Every 10ms |
| Cleanup / eviction | $O(P)$ | Every 1000ms |
| Heartbeat | $O(P)$ | Every 30s |
| Stats sync | $O(P + T)$ | Every 100ms |

### 6.2 Steady-State Memory

| Component | Space |
|-----------|-------|
| Peer tracking | $O(P)$ where $P \leq \text{max\_peers}$ |
| Trust system | $O(P)$ entries |
| Reliable queue | $O(P \cdot Q)$ where $Q$ = max queue depth |
| Audit log | $O(A)$ where $A$ = audit buffer size |
| Neural network | $O(S + N)$ |

**Total**: $O(P + S + N + A)$ — all bounded by configuration constants.

---

## References

- Kademlia: Maymounov et al., "Kademlia: A Peer-to-Peer Information System Based on the XOR Metric" (2002)
- XChaCha20-Poly1305: RFC 8439 (with extended nonce)
- Ed25519: RFC 8032
- Token Bucket: RFC 2697
