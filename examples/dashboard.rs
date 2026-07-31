//! Dashboard Example — Live Observability Dashboard
//!
//! Starts a single neuron-wire node with the live web dashboard.
//! Open http://localhost:9090 in your browser to see real-time metrics.
//!
//! Usage:
//!     cargo run --example dashboard [--port PORT]
//!
//! The dashboard shows:
//!   - DHT state and peer connections
//!   - Packet throughput (PPS, BPS)
//!   - System tick rate and CPU utilization
//!   - SSE live feed for real-time updates
//!   - Peer latency and trust scores

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    println!("═══ NEURON-WIRE DASHBOARD ═══");
    println!();

    // Create metrics registry
    let metrics = neuron_wire::observability::MetricsRegistry::new();

    // Create trace collector
    let trace_collector = Arc::new(Mutex::new(neuron_wire::observability::TraceCollector::new()));

    // Shutdown flag
    let shutdown = Arc::new(AtomicBool::new(false));

    // Dashboard config — listen on 9090
    let config = neuron_wire::observability::DashboardConfig {
        listen_addr: "0.0.0.0:9090".to_string(),
        html_path: None,
    };

    // Spawn dashboard server
    let dashboard_handle = neuron_wire::observability::spawn_dashboard(
        config,
        metrics.clone(),
        trace_collector.clone(),
        shutdown.clone(),
    );

    println!("📊 Dashboard:  http://localhost:9090");
    println!("📈 Metrics:    http://localhost:9090/metrics");
    println!("🔌 SSE feed:   http://localhost:9090/events");
    println!();
    println!("Simulating metrics... Press Ctrl+C to stop.");
    println!();

    // Simulate some metrics activity
    let mut tick: u64 = 0;
    for _ in 0..200 {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        tick += 1;

        // Increment counters
        metrics.inc_ticks();
        metrics.inc_packets_sent();
        metrics.inc_packets_recv();
        metrics.inc_bytes_sent(1024 + (tick % 256));
        metrics.inc_bytes_recv(512 + (tick % 128));

        // Alternating idle/busy
        if !tick.is_multiple_of(3) {
            metrics.inc_busy();
        } else {
            metrics.inc_idle();
        }

        // Set gauges with some variation
        metrics.set_peer_count(3 + (tick % 5));
        metrics.set_dht_peers(5 + (tick % 3));
        metrics.set_neuron_count(10 + (tick % 20));
        metrics.set_synapse_count(50 + (tick * 2 % 100));
        metrics.set_trust_avg(0.75 + (tick % 10) as f64 * 0.02);

        // Add a packet event every 5 ticks
        if tick.is_multiple_of(5) {
            metrics.push_packet_event(neuron_wire::observability::PacketEvent {
                seq: tick as u32,
                dir: if tick.is_multiple_of(2) {
                    "in".to_string()
                } else {
                    "out".to_string()
                },
                size: 128 + (tick as usize % 256),
                src: format!("192.168.1.{}", (tick % 5) + 1),
                dst: format!("127.0.0.1:{}", 9000 + (tick % 3)),
                is_reliable: tick.is_multiple_of(3),
                timestamp_ms: tick * 250,
            });
        }

        // Take snapshot every tick for history
        metrics.snapshot();

        thread::sleep(Duration::from_millis(250));
    }

    // Clean shutdown
    shutdown.store(true, Ordering::Relaxed);
    dashboard_handle.join().ok();
    println!("\nDashboard shut down.");
}
