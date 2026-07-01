# Tutorial 06: Observability Deep Dive

**Goal:** Understand NWP's three-pillar observability stack — real-time metrics,
Prometheus exposition, structured distributed tracing, and the SSE-powered live
dashboard.

**Estimated time:** 25 minutes

---

## 1. The Observability Stack

NWP's `src/observability/` module provides three complementary observability
pillars, each corresponding to a standard pillar of observability:

| Pillar | NWP Module | Standard | What It Tells You |
|--------|-----------|----------|-------------------|
| **Metrics** | `metrics.rs` | Prometheus / Grafana | Counters (packets sent/recv) and gauges (peer count, queue depth) |
| **Tracing** | `opentelemetry.rs` | OpenTelemetry / Jaeger | Per-tick span tree: UDP drain → compute → retransmit |
| **Dashboard** | `dashboard.rs` | Custom HTML+SSE | Live topology animation, charts, and event stream |

All three share the same data source: the `MetricsRegistry`, a thread-safe
`Arc`-wrapped struct that lives on the engine thread and is sampled every
~100 ticks (~100 ms at 1 ms/tick).

```
┌──────────────────────────────────────────────────┐
│                  Engine Loop                       │
│                                                    │
│  tick() → metrics.inc_ticks()                      │
│  recv() → metrics.inc_packets_recv()               │
│  send() → metrics.inc_packets_sent()               │
│  neural → metrics.set_neuron_count()               │
│                                                    │
│  Every SAMPLE_INTERVAL_TICKS (100):                │
│    metrics.snapshot() → append to history          │
│                                                    │
└────────────────────┬─────────────────────────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   MetricsRegistry     │
         │   (thread-safe Arc)   │
         └──┬──────────┬────────┘
            │          │
            ▼          ▼
   ┌────────────┐  ┌────────────┐
   │ Dashboard  │  │ Prometheus │
   │ HTTP + SSE │  │ /metrics   │
   │ :9090      │  │ :9090      │
   └────────────┘  └────────────┘
```

## 2. The MetricsRegistry (src/observability/metrics.rs)

The `MetricsRegistry` is the central data structure. It collects:

### Counters (lock-free AtomicU64)

```rust
// From metrics.rs
packets_sent: AtomicU64,
packets_recv: AtomicU64,
bytes_sent: AtomicU64,
bytes_recv: AtomicU64,
ticks_total: AtomicU64,
idle_ticks: AtomicU64,
busy_ticks: AtomicU64,
neurons_born: AtomicU64,
neurons_died: AtomicU64,
synapses_created: AtomicU64,
synapses_pruned: AtomicU64,
gossip_messages_sent: AtomicU64,
// ... plus security counters
handshake_successes: AtomicU64,
handshake_failures: AtomicU64,
replay_attacks: AtomicU64,
```

Counters are updated inline during the engine tick with `fetch_add(1, Relaxed)`
— no locking, no contention.

### Gauges (RwLock-protected)

```rust
// GaugeSet
peer_count: u64,
dht_peers: u64,
synapse_count: u64,
neuron_count: u64,
reliable_queue_depth: u64,
```

Gauges are set periodically by the DHT and neural subsystems. They're protected
by `RwLock` (read-heavy, write-rare — perfect for the dashboard reader pattern).

### History Buffer

A `VecDeque<MetricsSnapshot>` with `MAX_HISTORY = 300` entries. A snapshot is
taken every `SAMPLE_INTERVAL_TICKS = 100` ticks (~100 ms). The history buffer
provides:

- **60 seconds of lookback** (300 snapshots × 200 ms effective interval)
- **Per-snapshot throughput** (PPS, BPS calculated from deltas)
- **Tick rate** (Hz averaged over the sample window)

### Packet Events (Ring Buffer)

A ring buffer of the last 200 packet events, each containing:

```rust
pub struct PacketEvent {
    pub seq: u32,
    pub dir: String,       // "in" | "out"
    pub size: usize,
    pub src: String,
    pub dst: String,
    pub is_reliable: bool,
    pub timestamp_ms: u64,
}
```

### Peer Latencies

Per-peer latency tracking with EMA-smoothed RTT:

```rust
pub struct PeerLatencyStats {
    pub entity_id: [u8; 32],
    pub addr: String,
    pub rtt_ms: f32,
    pub packets_exchanged: u64,
    pub last_seen_ms: u64,
    pub trust_score: f32,
    pub is_trusted: bool,
}
```

## 3. Prometheus Endpoint (src/observability/prometheus.rs)

The dashboard server exposes `/metrics` in Prometheus text format:

```bash
# While a simulation is running with the dashboard, scrape metrics
curl -s http://localhost:9090/metrics
```

Example output:

```
# HELP neuron_wire_counters Counter values
# TYPE neuron_wire_counters counter
neuron_wire_packets_sent 482
neuron_wire_packets_recv 482
neuron_wire_ticks_total 30000
neuron_wire_neurons_born 12
neuron_wire_neurons_died 0

# HELP neuron_wire_gauges Gauge values
# TYPE neuron_wire_gauges gauge
neuron_wire_peer_count 9
neuron_wire_dht_peers 9
neuron_wire_synapse_count 256
neuron_wire_neuron_count 10

# HELP neuron_wire_peer_rtt Per-peer RTT in ms
# TYPE neuron_wire_peer_rtt gauge
neuron_wire_peer_rtt{addr="127_0_0_1_9002",trust="0.85"} 0.12
neuron_wire_peer_rtt{addr="127_0_0_1_9003",trust="0.85"} 0.15

# HELP neuron_wire_throughput_bytes_per_sec Instantaneous throughput
# TYPE neuron_wire_throughput_bytes_per_sec gauge
neuron_wire_bps_in 120.5 1728492000000
neuron_wire_bps_out 118.3 1728492000000
```

You can point a Prometheus server at this endpoint:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'neuron-wire'
    static_configs:
      - targets: ['localhost:9090']
```

## 4. OpenTelemetry Tracing (src/observability/opentelemetry.rs)

NWP includes a lightweight tracing system compatible with OpenTelemetry. Every
engine tick can produce a **span tree**:

```
Root: "engine_tick" (trace_id=abc123, span_id=def456)
  ├── Child: "phase1_udp_drain"
  │   └── attributes: { recv_count: 3, bytes_recv: 180 }
  ├── Child: "phase2_outbound_drain"
  │   └── attributes: { sent_count: 2 }
  ├── Child: "phase3_neural_compute"
  │   └── attributes: { neurons: 10, synapses: 256, surprise: 0.05 }
  ├── Child: "phase4_retransmit" (every 10th tick)
  └── Child: "phase5_apoptosis" (every 1000th tick)
```

### Span Data Structure

```rust
pub struct Span {
    pub name: String,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: String,
    pub start_time_ns: u128,
    pub end_time_ns: u128,
    pub attributes: HashMap<String, String>,
    pub status: String,       // "OK" | "ERROR" | "UNSET"
    pub status_description: String,
}
```

### Trace Collector

The built-in `TraceCollector` buffers spans in a ring buffer (max 10,000) and
can export them as JSON:

```rust
// Development mode — prints to stderr
collector.export_json();
// → [OTEL] spans: [{"name":"engine_tick",...}, ...]

// Dashboard mode — exposed via /api/spans
```

To connect to a real OTLP collector (Grafana Tempo, Jaeger, Datadog):

```rust
// Future: when opentelemetry crates are added as dependencies
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;

let exporter = opentelemetry_otlp::new_exporter()
    .tonic()
    .with_endpoint("http://tempo:4317");
```

## 5. Dashboard HTTP Server (src/observability/dashboard.rs)

The dashboard is a **single-threaded HTTP server** (`TcpListener` — no HTTP
framework or async runtime) that serves:

| Route | Content Type | Description |
|-------|-------------|-------------|
| `/` | `text/html` | Self-contained dashboard HTML |
| `/api/snapshot` | `application/json` | Latest `MetricsSnapshot` |
| `/api/history` | `application/json` | Full time-series history (300 entries) |
| `/api/packets` | `application/json` | Recent packet events (200 max) |
| `/api/peers` | `application/json` | Per-peer RTT/latency stats |
| `/api/spans` | `application/json` | Recent OTel spans |
| `/metrics` | `text/plain` | Prometheus text format |
| `/health` | `text/plain` | Health check (`OK`) |
| `/events` | `text/event-stream` | **SSE stream** for live updates |

### Server-Sent Events (SSE) Stream

The `/events` endpoint pushes real-time updates every ~100 ms:

```
event: snapshot
data: {"timestamp_ms":1234,"packets_sent":482,"peer_count":9,...}

event: packet
data: {"seq":42,"dir":"in","size":60,"src":"127.0.0.1:9002","dst":"127.0.0.1:9001"}

event: peer
data: {"addr":"127.0.0.1:9002","rtt_ms":0.12}
```

The dashboard HTML (`src/observability/dashboard.html`) uses the
[`EventSource` API](https://developer.mozilla.org/en-US/docs/Web/API/EventSource)
to consume this stream:

```javascript
const evtSource = new EventSource('/events');
evtSource.addEventListener('snapshot', (e) => {
    const data = JSON.parse(e.data);
    updateCharts(data);
    updateCounters(data);
});
evtSource.addEventListener('packet', (e) => {
    const pkt = JSON.parse(e.data);
    animatePacket(pkt);
});
```

## 6. Running with Observability

### Launch a simulation with the dashboard enabled

The dashboard server is spawned in a background thread when the engine starts.
It binds to `0.0.0.0:9090` by default:

```bash
# Run a simulation (dashboard auto-starts if observability is compiled in)
cargo run --release --example simulate -- --nodes 5 --duration 60 --output-dir results/obs-demo
```

In a **second terminal**, explore the APIs:

```bash
# Latest snapshot
curl -s http://localhost:9090/api/snapshot | python3 -m json.tool

# History (last 300 snapshots)
curl -s http://localhost:9090/api/history | python3 -m json.tool | head -50

# Raw Prometheus metrics
curl -s http://localhost:9090/metrics

# Health check
curl -s http://localhost:9090/health
# → OK
```

### Visualise metrics in real time

Open the dashboard in a browser:

```bash
# Option 1: Open the dashboard HTML directly (if served by the HTTP server)
open http://localhost:9090

# Option 2: Or open the standalone dashboard file
open dashboard/index.html
```

The dashboard shows:

- **Network topology animation** — force-directed graph with pulsing nodes
- **Live counters** — packets/sec, peer count, tick rate
- **Chart.js charts** — convergence time, bandwidth scaling, peer discovery
- **Stats row** — animated counters
- **Full results table** — all trials

### Scrape with Prometheus (optional)

If you have Prometheus running:

```yaml
# Add to prometheus.yml
scrape_configs:
  - job_name: 'neuron-wire'
    scrape_interval: 5s
    static_configs:
      - targets: ['localhost:9090']
```

Then query in PromQL:

```promql
# Packet rate
rate(neuron_wire_packets_sent[10s])

# Peer count
neuron_wire_peer_count

# Tick rate
neuron_wire_tick_rate_hz
```

## 7. Metrics in Action

Let's trace what happens in the metrics during a simulation:

### Tick 0–1000 (Discovery Phase)

```rust
// Engine loop — each call updates atomics
metrics.inc_ticks();       // tick++

// Phase 1: recv PING from peer
metrics.inc_packets_recv();
metrics.inc_bytes_recv(60);

// DHT: peer discovered
metrics.set_peer_count(1);
metrics.set_dht_peers(1);

// Every 100 ticks: snapshot()
let snap = metrics.snapshot();
// snap.peer_count = 1
// snap.packets_recv = 3
// snap.pps_in = 30.0 (3 pkts / 0.1s)
```

### Tick 1000–10000 (Steady State)

```rust
// Converged — periodic PING/PONG only
// ~2 packets/sec/node
metrics.inc_idle();        // Most ticks are idle
// metrics.busy_ticks is near 0
// idle_ratio ≈ 0.95 (95% idle)
```

### After 30 seconds

```bash
curl -s http://localhost:9090/api/snapshot | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'Uptime: {d[\"uptime_secs\"]}s')
print(f'Ticks: {d[\"ticks_total\"]} @ {d[\"tick_rate_hz\"]/1000:.0f} KHz')
print(f'Peers: {d[\"peer_count\"]}')
print(f'Idle ratio: {d[\"idle_ticks\"]/d[\"ticks_total\"]:.1%}')
print(f'Packets in: {d[\"pps_in\"]:.1f}/s, out: {d[\"pps_out\"]:.1f}/s')
"
```

Example output:

```
Uptime: 30s
Ticks: 30000 @ 1000 KHz
Peers: 9
Idle ratio: 94.2%
Packets in: 2.1/s, out: 2.1/s
```

## 8. Adding Custom Metrics

You can extend the metrics registry with domain-specific metrics:

```rust
// In your subsystem
use crate::observability::MetricsRegistry;

// During initialization
metrics.set_neuron_count(initial_neurons);

// During each tick
metrics.inc_neurons_born();   // Neurogenesis fired
metrics.inc_synapses_pruned(); // Micro-pruning removed a synapse
metrics.set_ml_metrics(        // ML metrics
    surprise,     // total_surprise
    curiosity,    // curiosity_drive
    avg_act,      // avg_activation
    grad_norm,    // gradient_norm
    lr,           // learning_rate
    forget,       // forgetting_rate
    distill,      // distillation_loss
);
```

All of these appear in the `/metrics` Prometheus endpoint, the `/api/snapshot`
JSON API, and the dashboard automatically.

## Summary

- ✅ **MetricsRegistry** provides lock-free counters and RwLock-protected gauges
  with a 300-entry time-series history
- ✅ **Prometheus endpoint** at `/metrics` exposes everything in standard text
  format for scraping
- ✅ **OpenTelemetry-compatible spans** track per-tick phases with parent-child
  relationships and attributes
- ✅ **Dashboard HTTP server** serves JSON APIs, HTML frontend, and a real-time
  **SSE stream** (`/events`) for live updates
- ✅ All three pillars share the same `MetricsRegistry` — no duplication
- ✅ Built with **zero external dependencies** — pure `std::net::TcpListener`
  and `std::sync::atomic`

---

*This concludes the Neuron Wire Protocol tutorial series. You now have a
complete understanding of the project — from cloning and building, through
multi-node networking and engine internals, to reproducible experiments and
observability.*

## Source Files Referenced

| File | Lines | Key Content |
|------|-------|-------------|
| [`src/observability/mod.rs`](../../src/observability/mod.rs) | 1–20 | Module structure, quick-start example |
| [`src/observability/metrics.rs`](../../src/observability/metrics.rs) | 1–555 | `MetricsRegistry`, `MetricsSnapshot`, counters and gauges |
| [`src/observability/prometheus.rs`](../../src/observability/prometheus.rs) | 1–57 | Prometheus text format exporter |
| [`src/observability/opentelemetry.rs`](../../src/observability/opentelemetry.rs) | 1–224 | `Span`, `TraceCollector`, span tree |
| [`src/observability/dashboard.rs`](../../src/observability/dashboard.rs) | 1–240 | HTTP server, SSE stream, all API routes |
| [`src/observability/dashboard.html`](../../src/observability/dashboard.html) | — | Self-contained dashboard HTML with Chart.js |
| [`dashboard/README.md`](../../dashboard/README.md) | 1–18 | Public dashboard deployment at neuron-wire-dashboard.vercel.app |
