//! Prometheus `/metrics` endpoint — text format exposition.
//!
//! Converts the metrics registry into Prometheus-compatible format:
//! https://prometheus.io/docs/instrumenting/exposition_formats/
#![allow(missing_docs)]

use crate::observability::metrics::MetricsRegistry;

/// Format all metrics as Prometheus exposition text.
pub fn format_metrics(registry: &MetricsRegistry) -> String {
    let counters = registry.counters_snapshot();
    let gauges = registry.gauges_snapshot();
    let prefix = "neuron_wire";

    let mut out = String::with_capacity(4096);

    // ── Counters ──────────────────────────────────────────────
    out.push_str("# HELP neuron_wire_counters Counter values\n");
    out.push_str("# TYPE neuron_wire_counters counter\n");
    for (name, value) in &counters {
        let key = format!("{}_{}", prefix, name);
        out.push_str(&format!("{} {}\n", key, value));
    }

    // ── Gauges ────────────────────────────────────────────────
    out.push_str("# HELP neuron_wire_gauges Gauge values\n");
    out.push_str("# TYPE neuron_wire_gauges gauge\n");
    for (name, value) in &gauges {
        let key = format!("{}_{}", prefix, name);
        out.push_str(&format!("{} {}\n", key, value));
    }

    // ── Per-peer metrics (gauge) ───────────────────────────────
    let peers = registry.get_peer_latencies();
    out.push_str("# HELP neuron_wire_peer_rtt Per-peer RTT in ms\n");
    out.push_str("# TYPE neuron_wire_peer_rtt gauge\n");
    for peer in &peers {
        let addr_safe = peer.addr.replace(['.', ':'], "_");
        out.push_str(&format!(
            "neuron_wire_peer_rtt{{addr=\"{}\",trust=\"{:.2}\"}} {}\n",
            addr_safe, peer.trust_score, peer.rtt_ms
        ));
    }

    // ── Throughput gauges (from snapshot) ──────────────────────
    if let Some(snap) = registry.get_history().last() {
        out.push_str("# HELP neuron_wire_throughput_bytes_per_sec Instantaneous throughput\n");
        out.push_str("# TYPE neuron_wire_throughput_bytes_per_sec gauge\n");
        out.push_str(&format!("neuron_wire_bps_in {} {}\n", snap.bps_in, snap.timestamp_ms));
        out.push_str(&format!("neuron_wire_bps_out {} {}\n", snap.bps_out, snap.timestamp_ms));
        out.push_str(&format!("neuron_wire_pps_in {} {}\n", snap.pps_in, snap.timestamp_ms));
        out.push_str(&format!("neuron_wire_pps_out {} {}\n", snap.pps_out, snap.timestamp_ms));
        out.push_str(&format!("neuron_wire_tick_rate_hz {} {}\n", snap.tick_rate_hz, snap.timestamp_ms));
    }

    out
}
