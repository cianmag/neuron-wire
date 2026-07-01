# Tutorial 02: Multi-Node Network

**Goal:** Launch three NWP nodes on localhost, watch DHT discovery in action,
understand how packets flow between peers, and inspect the routing table.

**Estimated time:** 20 minutes

---

## 1. How NWP Nodes Find Each Other

Neuron Wire uses a **modified Kademlia Distributed Hash Table (DHT)** for peer
discovery. Every node:

1. Generates a random **256-bit NodeId** at startup
2. Listens on a UDP port for incoming NWP frames
3. Periodically **pings** known peers to measure latency
4. **Responds to PING** with PONG frames (echoing the ping sequence number
   for RTT calculation)
5. **Responds to FIND_NODE** queries with its N nearest known peers
6. Maintains a **routing table** of 256 k-buckets, each sorted by latency

On localhost, there's no NAT, no firewall, no packet loss — so discovery is
fast and reliable. The simulator abstracts away the manual launch and connect
steps, but understanding what it does under the hood is essential.

## 2. Launch Three Nodes with the Simulator

The simulator is the easiest way to see multi-node behaviour:

```bash
cd neuron-wire

# Run a 3-node simulation for 30 seconds with verbose output
cargo run --release --example simulate -- --nodes 3 --duration 30 --output-dir results/3node-demo
```

This spawns **three engine threads**, each bound to a unique localhost port.
The first node (`node_000`) listens on the configured base port, the second
(`node_001`) on base+1, and the third (`node_002`) on base+2. All three are
injected into each other's peer lists so they can bootstrap immediately.

### Expected Console Output

```
[SIM] Launching 1 trials of 3 nodes for 30s...
[SIM] Trial 1/1 seed=42 (paper-mode)

[ENGINE] tick=1000 rate=987654Hz rx=2 pkts tx=2 pkts peers=2
[ENGINE] tick=2000 rate=1001000Hz rx=0 pkts tx=0 pkts peers=2
[ENGINE] tick=3000 rate=999000Hz rx=3 pkts tx=3 pkts peers=2
...
[ENGINE] tick=4000 rate=402879Hz rx=2 pkts tx=2 pkts peers=3
```

Each engine prints a status line every ~1000 ticks. The key fields:

| Field | Meaning | Healthy Range |
|-------|---------|---------------|
| `tick=N` | Total ticks completed | Increases by ~1000 per second |
| `rate=NHz` | Instantaneous tick rate | 400 KHz – 1.1 MHz |
| `rx=N pkts` | Packets received in sample | 0–10 (idle), 100+ (heavy gossip) |
| `tx=N pkts` | Packets transmitted in sample | Same as rx |
| `peers=N` | Unique peers in routing table | 0 → 2 (for 3 nodes) |

## 3. Inspect the Routing Table

The simulator writes routing data to a CSV file:

```bash
column -t -s, results/3node-demo/routing.csv | head -25
```

You'll see something like:

```
tick  node_index  peer_count  total_in_table  converged
0     0           0           0               false
100   0           1           1               false
200   0           2           2               true
300   0           2           2               true
...
0     1           0           0               false
100   1           1           1               false
200   1           2           2               true
...
0     2           0           0               false
100   2           1           1               false
200   2           1           1               false
300   2           2           2               true
```

Key observations:

- **All nodes converged** within ~300 ticks (300 ms at 1 ms/tick + gossip latency)
- Node 2 took slightly longer — it discovered one peer at tick 100, then the
  second at tick 300. This is normal; gossip propagation isn't instantaneous.
- After convergence, `peer_count = 2` for every node (each knows the other two).

## 4. Watch the Bandwidth

```bash
column -t -s, results/3node-demo/bandwidth.csv | head -15
```

```
tick  node_index  bytes_sent  bytes_recv  pkts_sent  pkts_recv
0     0           0           0           0          0
100   0           180         180         3          3
200   0           360         360         6          6
...
```

Each packet is ~60 bytes (16-byte transport header + 16-byte NWP header +
~28-byte FlatBuffer body). A converged 3-node network at idle sends
approximately 2–3 packets per second per node (periodic PING/PONG exchanges).

## 5. Packet Exchange in Detail

When nodes discover each other, this is the exact message sequence:

```
Node A                          Node B
  │                               │
  │  ── PING ──────────────────►  │  (A says: "hello, here's my NodeId")
  │  ◄── PONG ──────────────────  │  (B says: "hello back, here's my NodeId + RTT of A's PING")
  │                               │
  │  ── FIND_NODE ──────────────► │  (A asks: "who else do you know?")
  │  ◄── NODES (Node C addr) ──── │  (B says: "I know Node C at 127.0.0.1:9002")
  │                               │
  │  ── PING Node C ────────────► │  (A pings C to verify)
  │  ◄── PONG Node C ◄─────────── │  (C responds)
  │                               │
```

This is called **iterative routing** — node A walks the XOR distance toward
unknown nodes by asking its known peers. In a small network (3 nodes), this
completes in a single round-trip.

## 6. What Happens in the Engine During Discovery

Every tick, the engine processes incoming and outgoing messages. During
a discovery-heavy tick:

**Phase 1 (Drain UDP):**
```rust
// src/engine_loop.rs — simplified
while let Ok((len, src)) = socket.recv_from(&mut buf) {
    let header = TransportHeader::parse(&buf[..16]);
    ack_tracker.record(header.sequence_number);
    let msg = parse_nwp_frame(&buf[16..len]);
    match msg.msg_type {
        PING => dht_handler.handle_ping(sender_id, addr, ping_seq),
        PONG => dht_handler.handle_pong(sender_id, addr, ping_seq, rtt),
        FIND_NODE => {
            let nearest = routing_table.nearest_nodes(target_id, K);
            enqueue(NODES { target_id, entries: nearest });
        }
        NODES => {
            for entry in entries {
                enqueue(PING { target: entry.id });
            }
        }
        _ => {}
    }
}
```

**Phase 2 (Drain Outbound):**
Any packets enqueued by the DHT handler (PING, PONG, NODES) are sent over UDP.

**Phase 3 (Compute):**
Neural computation runs every tick — but in a fresh network with no data,
the forward pass processes only the initial neurons (if any).

## 7. Real Packet Capture (Optional)

You can watch the actual UDP packets with a network tool:

```bash
# Run in another terminal while the simulation is active
# Requires tshark or tcpdump
sudo tshark -i lo -f "udp portrange 9000-9010" -V 2>/dev/null | head -100
```

Or use a Rust-specific tool from the codebase:

```bash
# The adversary module can log packet traces
cargo run --release --example simulate -- --nodes 3 --duration 30 --adversary trace:1
```

## 8. Scaling Up: 5 Nodes, 10 Nodes

Try larger networks — observation time is the same, but the discovery pattern
changes:

```bash
# 5 nodes
cargo run --release --example simulate -- --nodes 5 --duration 25 --output-dir results/5node-demo

# 10 nodes
cargo run --release --example simulate -- --nodes 10 --duration 30 --output-dir results/10node-demo
```

For 5 nodes, expect:

- Convergence in ~3 seconds (same as 3 nodes — Kademlia scales logarithmically)
- Max peers = 4/4 (each node knows all others)
- Slightly higher bandwidth: ~4–5 packets/sec per node
- Avg peers (steady state): ~3.5–3.8 (occasional missed gossip)

For 10 nodes:

- Convergence in ~3–4 seconds
- Max peers = 9/9
- Bandwidth per node: ~8–10 packets/sec
- Avg peers: ~8.0–8.5

This **O(log N) convergence scaling** is Kademlia's key property — adding more
nodes doesn't significantly increase discovery time.

## 9. Common Pitfalls

| Issue | Symptom | Fix |
|-------|---------|-----|
| **Port conflict** | `bind: Address already in use` | Wait 30s or change `--base-port` in simulator config |
| **Firewall** | No packets exchanged | Check `localhost` only — real networks need NAT traversal |
| **Wrong binary** | `command not found: simulate` | Run from `neuron-wire/` directory with `cargo run --example simulate` |
| **Stale output** | Old CSV files | Delete `results/3node-demo/` or use fresh `--output-dir` |
| **Timing issues on Windows** | Lower tick rate | Expected — Windows timer resolution is ~15ms vs Linux ~1ms |

## Summary

You've now:

- ✅ Launched a 3-node NWP network on localhost
- ✅ Read engine status lines to understand tick rate, packet flow, and peer count
- ✅ Inspected CSV routing tables showing per-node convergence
- ✅ Understood the PING/PONG/FIND_NODE/NODES message sequence
- ✅ Run larger (5-node, 10-node) networks and observed O(log N) scaling

In **Tutorial 03**, you'll dive into the engine loop itself — the six phases
that make every tick deterministic, how backpressure works, and the difference
between real-time and simulated execution.

## Source Files Referenced

| File | Key Section |
|------|-------------|
| [`src/engine_loop.rs`](../../src/engine_loop.rs) | Main loop, Phase 1 (recv) and Phase 2 (send), DHT dispatch |
| [`src/dht.rs`](../../src/dht.rs) | `handle_ping()`, `handle_pong()`, `nearest_nodes()`, bootstrap |
| [`src/transport.rs`](../../src/transport.rs) | `TransportHeader` parsing, ACK tracking |
| [`src/simulator.rs`](../../src/simulator.rs) | `Simulator::launch()` — creates nodes, injects peers, starts threads |
| [`src/types.rs`](../../src/types.rs) | `MsgType` enum — PING=7, PONG=8, FIND_NODE=9, NODES=10 |
