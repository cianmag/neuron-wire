//! Central metrics registry for the NWP observability stack.
//!
//! Thread-safe counters, gauges, histograms, and a time-series history buffer
//! that feeds the web dashboard, Prometheus exporter, and OpenTelemetry spans.
#![allow(missing_docs)]

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::components::EntityId;

// ─── Constants ─────────────────────────────────────────────────

/// How many historical snapshots to keep (e.g. 60s at 1s intervals).
pub const MAX_HISTORY: usize = 300;

/// How often (in engine ticks) to sample a snapshot.
pub const SAMPLE_INTERVAL_TICKS: u64 = 100; // ~100ms at 1ms ticks

/// Maximum number of peer latency entries to track.
pub const MAX_PEER_LATENCIES: usize = 500;

/// Maximum packet events in the ring buffer.
pub const MAX_PACKET_EVENTS: usize = 200;

// ─── Core Metrics ──────────────────────────────────────────────

/// Thread-safe metrics registry shared between engine and dashboard.
#[derive(Clone)]
pub struct MetricsRegistry {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    // ── Counters (atomic, lock-free) ─────────────────────────
    packets_sent: AtomicU64,
    packets_recv: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_recv: AtomicU64,
    ticks_total: AtomicU64,
    idle_ticks: AtomicU64,
    busy_ticks: AtomicU64,
    rate_limited_dropped: AtomicU64,
    handshake_successes: AtomicU64,
    handshake_failures: AtomicU64,
    invalid_signatures: AtomicU64,
    replay_attacks: AtomicU64,
    audit_entries_total: AtomicU64,
    neurons_born: AtomicU64,
    neurons_died: AtomicU64,
    synapses_created: AtomicU64,
    synapses_pruned: AtomicU64,
    gossip_messages_sent: AtomicU64,

    // ── Gauges (mutex-protected for complex state) ────────────
    gauges: RwLock<GaugeSet>,
    // ── Time-series history ────────────────────────────────────
    history: RwLock<VecDeque<MetricsSnapshot>>,
    // ── Packet event ring buffer ───────────────────────────────
    packet_events: RwLock<VecDeque<PacketEvent>>,
    // ── Peer latencies ─────────────────────────────────────────
    peer_latencies: RwLock<HashMap<EntityId, PeerLatencyStats>>,
    // ── Bandwidth tracking ─────────────────────────────────────
    /// Bandwidth sample history for throughput calculations (future use)
    #[allow(dead_code)]
    bw_samples: RwLock<VecDeque<BwSample>>,
    // ── ML metrics ─────────────────────────────────────────────
    ml_metrics: RwLock<MlMetricSet>,
    // ── Startup time ───────────────────────────────────────────
    started_at: Instant,
}

struct GaugeSet {
    peer_count: u64,
    dht_peers: u64,
    synapse_count: u64,
    neuron_count: u64,
    reliable_queue_depth: u64,
    session_count: u64,
    audit_buffered: u64,
    trust_score_avg: f64,
    sybil_peers: u64,
    rate_limited_peers: u64,
    auth_failures: u64,
    encrypted_packets: u64,
    peer_capacity_ratio: f64,
    active_sessions: u64,
    ephemeral_sessions: u64,
    max_peers: u64,
}

impl Default for GaugeSet {
    fn default() -> Self {
        GaugeSet {
            peer_count: 0,
            dht_peers: 0,
            synapse_count: 0,
            neuron_count: 0,
            reliable_queue_depth: 0,
            session_count: 0,
            audit_buffered: 0,
            trust_score_avg: 0.0,
            sybil_peers: 0,
            rate_limited_peers: 0,
            auth_failures: 0,
            encrypted_packets: 0,
            peer_capacity_ratio: 0.0,
            active_sessions: 0,
            ephemeral_sessions: 0,
            max_peers: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct MlMetricSet {
    pub total_surprise: f64,
    pub curiosity_drive: f64,
    pub avg_activation: f64,
    pub gradient_norm: f64,
    pub learning_rate: f64,
    pub forgetting_rate: f64,
    pub distillation_loss: f64,
}

impl Default for MlMetricSet {
    fn default() -> Self {
        MlMetricSet {
            total_surprise: 0.0,
            curiosity_drive: 0.0,
            avg_activation: 0.0,
            gradient_norm: 0.0,
            learning_rate: 0.01,
            forgetting_rate: 0.0,
            distillation_loss: 0.0,
        }
    }
}

/// A time-indexed snapshot of all metrics at a point in time.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    /// Timestamp (ms since registry creation)
    pub timestamp_ms: u64,
    // ── Counters ──────────────────────────────────────────────
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub ticks_total: u64,
    pub idle_ticks: u64,
    pub busy_ticks: u64,
    pub rate_limited_dropped: u64,
    pub handshake_successes: u64,
    pub handshake_failures: u64,
    pub invalid_signatures: u64,
    pub replay_attacks: u64,
    pub audit_entries_total: u64,
    pub neurons_born: u64,
    pub neurons_died: u64,
    pub synapses_created: u64,
    pub synapses_pruned: u64,
    pub gossip_messages: u64,
    // ── Gauges ────────────────────────────────────────────────
    pub peer_count: u64,
    pub dht_peers: u64,
    pub synapse_count: u64,
    pub neuron_count: u64,
    pub reliable_queue_depth: u64,
    pub session_count: u64,
    pub audit_buffered: u64,
    pub trust_score_avg: f64,
    pub sybil_peers: u64,
    pub rate_limited_peers: u64,
    // ── Security (from EngineStats) ─────────────────────────
    pub auth_failures: u64,
    pub encrypted_packets: u64,
    // ── Capacity (from EngineStats) ─────────────────────────
    pub peer_capacity_ratio: f64,
    pub active_sessions: u64,
    pub ephemeral_sessions: u64,
    pub max_peers: u64,
    // ── Throughput (delta from previous snapshot) ─────────────
    pub pps_in: f64,
    pub pps_out: f64,
    pub bps_in: f64,
    pub bps_out: f64,
    // ── ML metrics ────────────────────────────────────────────
    pub total_surprise: f64,
    pub curiosity_drive: f64,
    pub avg_activation: f64,
    pub gradient_norm: f64,
    pub learning_rate: f64,
    pub forgetting_rate: f64,
    pub distillation_loss: f64,
    // ── Derived ───────────────────────────────────────────────
    pub tick_rate_hz: f64,
    pub uptime_secs: u64,
}

/// A single packet event for the live animation stream.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PacketEvent {
    pub seq: u32,
    pub dir: String, // "in" | "out"
    pub size: usize,
    pub src: String,
    pub dst: String,
    pub is_reliable: bool,
    pub timestamp_ms: u64,
}

/// Per-peer latency stats.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PeerLatencyStats {
    pub entity_id: [u8; 32],
    pub addr: String,
    pub rtt_ms: f32,
    pub packets_exchanged: u64,
    pub last_seen_ms: u64,
    pub trust_score: f32,
    pub is_trusted: bool,
}

/// Bandwidth sample for throughput calculation (future use)
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct BwSample {
    time: Instant,
    bytes_sent: u64,
    bytes_recv: u64,
    packets_sent: u64,
    packets_recv: u64,
}

impl MetricsRegistry {
    /// Create a new empty metrics registry.
    pub fn new() -> Self {
        let mut history = VecDeque::with_capacity(MAX_HISTORY);
        history.push_back(MetricsSnapshot::default());

        MetricsRegistry {
            inner: Arc::new(MetricsInner {
                packets_sent: AtomicU64::new(0),
                packets_recv: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_recv: AtomicU64::new(0),
                ticks_total: AtomicU64::new(0),
                idle_ticks: AtomicU64::new(0),
                busy_ticks: AtomicU64::new(0),
                rate_limited_dropped: AtomicU64::new(0),
                handshake_successes: AtomicU64::new(0),
                handshake_failures: AtomicU64::new(0),
                invalid_signatures: AtomicU64::new(0),
                replay_attacks: AtomicU64::new(0),
                audit_entries_total: AtomicU64::new(0),
                neurons_born: AtomicU64::new(0),
                neurons_died: AtomicU64::new(0),
                synapses_created: AtomicU64::new(0),
                synapses_pruned: AtomicU64::new(0),
                gossip_messages_sent: AtomicU64::new(0),
                gauges: RwLock::new(GaugeSet::default()),
                history: RwLock::new(history),
                packet_events: RwLock::new(VecDeque::with_capacity(MAX_PACKET_EVENTS)),
                peer_latencies: RwLock::new(HashMap::with_capacity(MAX_PEER_LATENCIES)),
                bw_samples: RwLock::new(VecDeque::with_capacity(10)),
                ml_metrics: RwLock::new(MlMetricSet::default()),
                started_at: Instant::now(),
            }),
        }
    }

    // ── Counter increment helpers ──────────────────────────────

    pub fn inc_packets_sent(&self) {
        self.inner.packets_sent.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_packets_recv(&self) {
        self.inner.packets_recv.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_bytes_sent(&self, n: u64) {
        self.inner.bytes_sent.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_bytes_recv(&self, n: u64) {
        self.inner.bytes_recv.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_ticks(&self) {
        self.inner.ticks_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_idle(&self) {
        self.inner.idle_ticks.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_busy(&self) {
        self.inner.busy_ticks.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_rate_limited(&self) {
        self.inner
            .rate_limited_dropped
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_handshake_ok(&self) {
        self.inner
            .handshake_successes
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_handshake_fail(&self) {
        self.inner
            .handshake_failures
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_bad_sig(&self) {
        self.inner
            .invalid_signatures
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_replay(&self) {
        self.inner.replay_attacks.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_audit_entry(&self) {
        self.inner
            .audit_entries_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_neurons_born(&self) {
        self.inner.neurons_born.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_neurons_died(&self) {
        self.inner.neurons_died.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_synapses_created(&self) {
        self.inner.synapses_created.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_synapses_pruned(&self) {
        self.inner.synapses_pruned.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_gossip(&self) {
        self.inner
            .gossip_messages_sent
            .fetch_add(1, Ordering::Relaxed);
    }

    // ── Gauge setters ──────────────────────────────────────────

    pub fn set_peer_count(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.peer_count = v;
        }
    }
    pub fn set_dht_peers(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.dht_peers = v;
        }
    }
    pub fn set_synapse_count(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.synapse_count = v;
        }
    }
    pub fn set_neuron_count(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.neuron_count = v;
        }
    }
    pub fn set_reliable_queue_depth(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.reliable_queue_depth = v;
        }
    }
    pub fn set_session_count(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.session_count = v;
        }
    }
    pub fn set_audit_buffered(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.audit_buffered = v;
        }
    }
    pub fn set_trust_avg(&self, v: f64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.trust_score_avg = v;
        }
    }
    pub fn set_sybil_peers(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.sybil_peers = v;
        }
    }
    pub fn set_rate_limited_peers(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.rate_limited_peers = v;
        }
    }
    pub fn set_auth_failures(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.auth_failures = v;
        }
    }
    pub fn set_encrypted_packets(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.encrypted_packets = v;
        }
    }
    pub fn set_peer_capacity(&self, ratio: f64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.peer_capacity_ratio = ratio;
        }
    }
    pub fn set_active_sessions(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.active_sessions = v;
        }
    }
    pub fn set_ephemeral_sessions(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.ephemeral_sessions = v;
        }
    }
    pub fn set_max_peers(&self, v: u64) {
        if let Ok(mut g) = self.inner.gauges.write() {
            g.max_peers = v;
        }
    }

    // ── ML metrics ─────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)] // telemetry fields, kept flat for ergonomics
    pub fn set_ml_metrics(
        &self,
        surprise: f64,
        curiosity: f64,
        avg_act: f64,
        grad_norm: f64,
        lr: f64,
        forget: f64,
        distill: f64,
    ) {
        if let Ok(mut m) = self.inner.ml_metrics.write() {
            m.total_surprise = surprise;
            m.curiosity_drive = curiosity;
            m.avg_activation = avg_act;
            m.gradient_norm = grad_norm;
            m.learning_rate = lr;
            m.forgetting_rate = forget;
            m.distillation_loss = distill;
        }
    }

    // ── Packet events ──────────────────────────────────────────

    pub fn push_packet_event(&self, event: PacketEvent) {
        if let Ok(mut pe) = self.inner.packet_events.write() {
            if pe.len() >= MAX_PACKET_EVENTS {
                pe.pop_front();
            }
            pe.push_back(event);
        }
    }

    // ── Peer latencies ─────────────────────────────────────────

    pub fn update_peer_latency(
        &self,
        eid: EntityId,
        addr: &str,
        rtt: f32,
        trust: f32,
        trusted: bool,
    ) {
        if let Ok(mut pl) = self.inner.peer_latencies.write() {
            if pl.len() >= MAX_PEER_LATENCIES {
                pl.clear();
            }
            let entry = pl.entry(eid).or_insert(PeerLatencyStats {
                entity_id: eid.0,
                addr: addr.to_string(),
                rtt_ms: 0.0,
                packets_exchanged: 0,
                last_seen_ms: 0,
                trust_score: 0.0,
                is_trusted: false,
            });
            entry.rtt_ms = rtt;
            entry.packets_exchanged += 1;
            entry.last_seen_ms = elapsed_ms(&self.inner.started_at);
            entry.trust_score = trust;
            entry.is_trusted = trusted;
            if entry.addr.is_empty() {
                entry.addr = addr.to_string();
            }
        }
    }

    // ── Snapshot ───────────────────────────────────────────────

    /// Take a snapshot of all current metrics and append to history.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let now = elapsed_ms(&self.inner.started_at);
        let gauges = self
            .inner
            .gauges
            .read()
            .ok()
            .map(|g| GaugeSet {
                peer_count: g.peer_count,
                dht_peers: g.dht_peers,
                synapse_count: g.synapse_count,
                neuron_count: g.neuron_count,
                reliable_queue_depth: g.reliable_queue_depth,
                session_count: g.session_count,
                audit_buffered: g.audit_buffered,
                trust_score_avg: g.trust_score_avg,
                sybil_peers: g.sybil_peers,
                rate_limited_peers: g.rate_limited_peers,
                // Security & capacity (from EngineStats via shared pointer)
                auth_failures: g.auth_failures,
                encrypted_packets: g.encrypted_packets,
                peer_capacity_ratio: g.peer_capacity_ratio,
                active_sessions: g.active_sessions,
                ephemeral_sessions: g.ephemeral_sessions,
                max_peers: g.max_peers,
            })
            .unwrap_or_default();
        let ml = self
            .inner
            .ml_metrics
            .read()
            .ok()
            .map(|m| MlMetricSet {
                total_surprise: m.total_surprise,
                curiosity_drive: m.curiosity_drive,
                avg_activation: m.avg_activation,
                gradient_norm: m.gradient_norm,
                learning_rate: m.learning_rate,
                forgetting_rate: m.forgetting_rate,
                distillation_loss: m.distillation_loss,
            })
            .unwrap_or_default();

        let prev = self
            .inner
            .history
            .read()
            .ok()
            .and_then(|h| h.back().cloned())
            .unwrap_or_default();

        let p_sent = self.inner.packets_sent.load(Ordering::Relaxed);
        let p_recv = self.inner.packets_recv.load(Ordering::Relaxed);
        let b_sent = self.inner.bytes_sent.load(Ordering::Relaxed);
        let b_recv = self.inner.bytes_recv.load(Ordering::Relaxed);
        let ticks = self.inner.ticks_total.load(Ordering::Relaxed);
        let idle = self.inner.idle_ticks.load(Ordering::Relaxed);
        let busy = self.inner.busy_ticks.load(Ordering::Relaxed);

        let dt_secs = if prev.timestamp_ms > 0 {
            (now - prev.timestamp_ms) as f64 / 1000.0
        } else {
            1.0
        };

        let snap = MetricsSnapshot {
            timestamp_ms: now,
            packets_sent: p_sent,
            packets_recv: p_recv,
            bytes_sent: b_sent,
            bytes_recv: b_recv,
            ticks_total: ticks,
            idle_ticks: idle,
            busy_ticks: busy,
            rate_limited_dropped: self.inner.rate_limited_dropped.load(Ordering::Relaxed),
            handshake_successes: self.inner.handshake_successes.load(Ordering::Relaxed),
            handshake_failures: self.inner.handshake_failures.load(Ordering::Relaxed),
            invalid_signatures: self.inner.invalid_signatures.load(Ordering::Relaxed),
            replay_attacks: self.inner.replay_attacks.load(Ordering::Relaxed),
            audit_entries_total: self.inner.audit_entries_total.load(Ordering::Relaxed),
            neurons_born: self.inner.neurons_born.load(Ordering::Relaxed),
            neurons_died: self.inner.neurons_died.load(Ordering::Relaxed),
            synapses_created: self.inner.synapses_created.load(Ordering::Relaxed),
            synapses_pruned: self.inner.synapses_pruned.load(Ordering::Relaxed),
            gossip_messages: self.inner.gossip_messages_sent.load(Ordering::Relaxed),
            peer_count: gauges.peer_count,
            dht_peers: gauges.dht_peers,
            synapse_count: gauges.synapse_count,
            neuron_count: gauges.neuron_count,
            reliable_queue_depth: gauges.reliable_queue_depth,
            session_count: gauges.session_count,
            audit_buffered: gauges.audit_buffered,
            trust_score_avg: gauges.trust_score_avg,
            sybil_peers: gauges.sybil_peers,
            rate_limited_peers: gauges.rate_limited_peers,
            auth_failures: gauges.auth_failures,
            encrypted_packets: gauges.encrypted_packets,
            peer_capacity_ratio: gauges.peer_capacity_ratio,
            active_sessions: gauges.active_sessions,
            ephemeral_sessions: gauges.ephemeral_sessions,
            max_peers: gauges.max_peers,
            pps_in: (p_recv - prev.packets_recv) as f64 / dt_secs.max(0.001),
            pps_out: (p_sent - prev.packets_sent) as f64 / dt_secs.max(0.001),
            bps_in: (b_recv - prev.bytes_recv) as f64 / dt_secs.max(0.001),
            bps_out: (b_sent - prev.bytes_sent) as f64 / dt_secs.max(0.001),
            total_surprise: ml.total_surprise,
            curiosity_drive: ml.curiosity_drive,
            avg_activation: ml.avg_activation,
            gradient_norm: ml.gradient_norm,
            learning_rate: ml.learning_rate,
            forgetting_rate: ml.forgetting_rate,
            distillation_loss: ml.distillation_loss,
            tick_rate_hz: if now > 0 {
                ticks as f64 / (now as f64 / 1000.0)
            } else {
                0.0
            },
            uptime_secs: now / 1000,
        };

        // Append to history
        if let Ok(mut h) = self.inner.history.write() {
            if h.len() >= MAX_HISTORY {
                h.pop_front();
            }
            h.push_back(snap.clone());
        }

        snap
    }

    /// Get the full history buffer.
    pub fn get_history(&self) -> Vec<MetricsSnapshot> {
        self.inner
            .history
            .read()
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get recent packet events.
    pub fn get_packet_events(&self) -> Vec<PacketEvent> {
        self.inner
            .packet_events
            .read()
            .map(|pe| pe.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Get peer latencies.
    pub fn get_peer_latencies(&self) -> Vec<PeerLatencyStats> {
        self.inner
            .peer_latencies
            .read()
            .map(|pl| pl.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get raw counter values for Prometheus export.
    pub fn counters_snapshot(&self) -> HashMap<&'static str, u64> {
        let mut m = HashMap::new();
        m.insert(
            "packets_sent",
            self.inner.packets_sent.load(Ordering::Relaxed),
        );
        m.insert(
            "packets_recv",
            self.inner.packets_recv.load(Ordering::Relaxed),
        );
        m.insert("bytes_sent", self.inner.bytes_sent.load(Ordering::Relaxed));
        m.insert("bytes_recv", self.inner.bytes_recv.load(Ordering::Relaxed));
        m.insert(
            "ticks_total",
            self.inner.ticks_total.load(Ordering::Relaxed),
        );
        m.insert("idle_ticks", self.inner.idle_ticks.load(Ordering::Relaxed));
        m.insert("busy_ticks", self.inner.busy_ticks.load(Ordering::Relaxed));
        m.insert(
            "rate_limited_dropped",
            self.inner.rate_limited_dropped.load(Ordering::Relaxed),
        );
        m.insert(
            "handshake_successes",
            self.inner.handshake_successes.load(Ordering::Relaxed),
        );
        m.insert(
            "handshake_failures",
            self.inner.handshake_failures.load(Ordering::Relaxed),
        );
        m.insert(
            "invalid_signatures",
            self.inner.invalid_signatures.load(Ordering::Relaxed),
        );
        m.insert(
            "replay_attacks",
            self.inner.replay_attacks.load(Ordering::Relaxed),
        );
        m.insert(
            "audit_entries_total",
            self.inner.audit_entries_total.load(Ordering::Relaxed),
        );
        m.insert(
            "neurons_born",
            self.inner.neurons_born.load(Ordering::Relaxed),
        );
        m.insert(
            "neurons_died",
            self.inner.neurons_died.load(Ordering::Relaxed),
        );
        m.insert(
            "synapses_created",
            self.inner.synapses_created.load(Ordering::Relaxed),
        );
        m.insert(
            "synapses_pruned",
            self.inner.synapses_pruned.load(Ordering::Relaxed),
        );
        m.insert(
            "gossip_messages",
            self.inner.gossip_messages_sent.load(Ordering::Relaxed),
        );
        m
    }

    /// Get gauge values for Prometheus export.
    pub fn gauges_snapshot(&self) -> HashMap<&'static str, f64> {
        let mut m = HashMap::new();
        if let Ok(g) = self.inner.gauges.read() {
            m.insert("peer_count", g.peer_count as f64);
            m.insert("dht_peers", g.dht_peers as f64);
            m.insert("synapse_count", g.synapse_count as f64);
            m.insert("neuron_count", g.neuron_count as f64);
            m.insert("reliable_queue_depth", g.reliable_queue_depth as f64);
            m.insert("session_count", g.session_count as f64);
            m.insert("audit_buffered", g.audit_buffered as f64);
            m.insert("trust_score_avg", g.trust_score_avg);
            m.insert("sybil_peers", g.sybil_peers as f64);
            m.insert("rate_limited_peers", g.rate_limited_peers as f64);
        }
        m
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        MetricsSnapshot {
            timestamp_ms: 0,
            packets_sent: 0,
            packets_recv: 0,
            bytes_sent: 0,
            bytes_recv: 0,
            ticks_total: 0,
            idle_ticks: 0,
            busy_ticks: 0,
            rate_limited_dropped: 0,
            handshake_successes: 0,
            handshake_failures: 0,
            invalid_signatures: 0,
            replay_attacks: 0,
            audit_entries_total: 0,
            neurons_born: 0,
            neurons_died: 0,
            synapses_created: 0,
            synapses_pruned: 0,
            gossip_messages: 0,
            peer_count: 0,
            dht_peers: 0,
            synapse_count: 0,
            neuron_count: 0,
            reliable_queue_depth: 0,
            session_count: 0,
            audit_buffered: 0,
            trust_score_avg: 0.0,
            sybil_peers: 0,
            rate_limited_peers: 0,
            auth_failures: 0,
            encrypted_packets: 0,
            peer_capacity_ratio: 0.0,
            active_sessions: 0,
            ephemeral_sessions: 0,
            max_peers: 0,
            pps_in: 0.0,
            pps_out: 0.0,
            bps_in: 0.0,
            bps_out: 0.0,
            total_surprise: 0.0,
            curiosity_drive: 0.0,
            avg_activation: 0.0,
            gradient_norm: 0.0,
            learning_rate: 0.01,
            forgetting_rate: 0.0,
            distillation_loss: 0.0,
            tick_rate_hz: 0.0,
            uptime_secs: 0,
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────

fn elapsed_ms(start: &Instant) -> u64 {
    start.elapsed().as_millis() as u64
}
