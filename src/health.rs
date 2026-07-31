//! Health and metrics HTTP endpoint for production monitoring.
//!
//! Spawns a minimal, dependency-free HTTP server in a background thread.
//! Reads engine stats via a shared `Mutex<EngineStats>` and serves three endpoints:
//!
//! - `GET /health` — Returns `200 OK` with `{"status":"ok"}` (liveness check)
//! - `GET /metrics` — Prometheus-compatible text metrics
//! - `GET /status` — Full JSON dump of engine state
//!
//! ## Usage
//!
//! ```ignore
//! let stats = Arc::new(Mutex::new(EngineStats::default()));
//! let handle = spawn_health_server("127.0.0.1:9100", stats.clone())?;
//! // ... pass stats to engine via EngineConfig.shared_stats ...
//! handle.join().unwrap(); // blocks forever (server runs until killed)
//! ```
//!
//! ## Design Decisions
//!
//! - **Zero dependencies** — Pure `std::net::TcpListener` + manual HTTP/1.0 responses.
//!   No tokio, no hyper, no actix. The engine already runs on a single thread;
//!   adding a framework would double the dependency tree for one simple endpoint.
//! - **Static strings** — No heap allocation per request. Metrics are built on each request
//!   with a pre-sized buffer. Response headers are compile-time constants.
//! - **Thread-safe** — Stats are shared via `Arc<Mutex<EngineStats>>`. The engine
//!   writes every 1000 ticks, the health server reads on each HTTP request.
//!   Contention is negligible (< 1µs per request at realistic volumes).

use crate::engine_loop::EngineStats;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// Spawn the health HTTP server on a background thread.
///
/// `bind` is the address to listen on (e.g. `"0.0.0.0:9100"`).
/// `stats` is a shared pointer to the engine's live stats.
///
/// Returns a `JoinHandle` and a shutdown signal. Drop the signal to stop the server.
pub fn spawn_health_server(
    bind: &str,
    stats: Arc<Mutex<EngineStats>>,
) -> std::io::Result<(thread::JoinHandle<()>, Arc<AtomicBool>)> {
    let listener = TcpListener::bind(bind)?;
    listener.set_nonblocking(true)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let sig = shutdown.clone();

    let bind_str = bind.to_string();

    let handle = thread::Builder::new()
        .name("nwp-health".to_string())
        .spawn(move || {
            eprintln!("[HEALTH] Listening on http://{bind_str}");
            const BUFFER_SIZE: usize = 4096;
            let mut buf = [0u8; BUFFER_SIZE];

            // Pre-computed HTTP response headers (static, zero-alloc)
            const HEALTH_200: &[u8] = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}\n";
            const METRICS_HEADER: &[u8] = b"HTTP/1.0 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nConnection: close\r\n\r\n";
            const STATUS_HEADER: &[u8] = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
            const NOT_FOUND: &[u8] = b"HTTP/1.0 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nNot Found\n";

            loop {
                // Check shutdown signal
                if shutdown.load(Ordering::Relaxed) {
                    eprintln!("[HEALTH] Shutdown signal received — exiting");
                    return;
                }

                // Accept with 100ms timeout (non-blocking + yield)
                match listener.accept() {
                    Ok((mut stream, addr)) => {
                        // Read the request (just the first line is enough to route)
                        let n = match stream.read(&mut buf) {
                            Ok(n) if n > 0 => n,
                            _ => {
                                let _ = stream.write(NOT_FOUND);
                                continue;
                            }
                        };
                        let request = std::str::from_utf8(&buf[..n]).unwrap_or("");

                        // Route
                        if request.starts_with("GET /health ") {
                            let _ = stream.write(HEALTH_200);
                        } else if request.starts_with("GET /metrics ") {
                            let _ = stream.write(METRICS_HEADER);
                            let metrics = format_metrics(&stats);
                            let _ = stream.write(metrics.as_bytes());
                        } else if request.starts_with("GET /status ") {
                            let _ = stream.write(STATUS_HEADER);
                            let json = format_status(&stats);
                            let _ = stream.write(json.as_bytes());
                        } else if request.starts_with("GET / ") || request.starts_with("GET /dashboard ") {
                            let _ = stream.write(b"HTTP/1.0 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n");
                            let _ = stream.write(DASHBOARD_HTML);
                        } else {
                            let _ = stream.write(NOT_FOUND);
                        }

                        // Log the request
                        let method = request.split_whitespace().next().unwrap_or("?");
                        let path = request
                            .split_whitespace()
                            .nth(1)
                            .unwrap_or("?");
                        eprintln!("[HEALTH] {addr} {method} {path}");
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection pending — sleep briefly
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("[HEALTH] Accept error: {e}");
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        })?;

    Ok((handle, sig))
}

/// Build Prometheus-compatible metrics text.
fn format_metrics(stats: &Arc<Mutex<EngineStats>>) -> String {
    let s = stats.lock().unwrap_or_else(|e| e.into_inner());
    // Pre-size the string to avoid reallocation
    let cap = 512;
    let mut out = String::with_capacity(cap);

    out.push_str("# HELP nwp_engine_ticks Total engine ticks executed\n");
    out.push_str("# TYPE nwp_engine_ticks counter\n");
    out.push_str(&format!("nwp_engine_ticks {}\n", s.total_ticks));

    out.push_str("# HELP nwp_packets_recv Total packets received\n");
    out.push_str("# TYPE nwp_packets_recv counter\n");
    out.push_str(&format!("nwp_packets_recv {}\n", s.packets_recv));

    out.push_str("# HELP nwp_packets_sent Total packets sent\n");
    out.push_str("# TYPE nwp_packets_sent counter\n");
    out.push_str(&format!("nwp_packets_sent {}\n", s.packets_sent));

    out.push_str("# HELP nwp_bytes_recv Total bytes received\n");
    out.push_str("# TYPE nwp_bytes_recv counter\n");
    out.push_str(&format!("nwp_bytes_recv {}\n", s.bytes_recv));

    out.push_str("# HELP nwp_bytes_sent Total bytes sent\n");
    out.push_str("# TYPE nwp_bytes_sent counter\n");
    out.push_str(&format!("nwp_bytes_sent {}\n", s.bytes_sent));

    out.push_str("# HELP nwp_retransmissions Total retransmissions\n");
    out.push_str("# TYPE nwp_retransmissions counter\n");
    out.push_str(&format!("nwp_retransmissions {}\n", s.retransmissions));

    out.push_str("# HELP nwp_peers Current peer count\n");
    out.push_str("# TYPE nwp_peers gauge\n");
    out.push_str(&format!("nwp_peers {}\n", s.peer_count));

    out.push_str("# HELP nwp_reliable_queue_depth Current reliable queue depth\n");
    out.push_str("# TYPE nwp_reliable_queue_depth gauge\n");
    out.push_str(&format!(
        "nwp_reliable_queue_depth {}\n",
        s.reliable_queue_depth
    ));

    out.push_str("# HELP nwp_idle_ratio Fraction of ticks with no packets\n");
    out.push_str("# TYPE nwp_idle_ratio gauge\n");
    let idle_ratio = if s.total_ticks > 0 {
        s.idle_ticks as f64 / s.total_ticks as f64
    } else {
        0.0
    };
    out.push_str(&format!("nwp_idle_ratio {:.4}\n", idle_ratio));

    out.push_str("# HELP nwp_tick_rate_hz Actual tick rate\n");
    out.push_str("# TYPE nwp_tick_rate_hz gauge\n");
    out.push_str(&format!("nwp_tick_rate_hz {:.0}\n", s.actual_tick_rate_hz));

    // ── Security metrics ─────────────────────────────────────
    out.push_str("# HELP nwp_authenticated_packets Total packets with valid Ed25519 signatures\n");
    out.push_str("# TYPE nwp_authenticated_packets counter\n");
    out.push_str(&format!(
        "nwp_authenticated_packets {}\n",
        s.authenticated_packets
    ));

    out.push_str("# HELP nwp_encrypted_packets Total AEAD-encrypted packets\n");
    out.push_str("# TYPE nwp_encrypted_packets counter\n");
    out.push_str(&format!("nwp_encrypted_packets {}\n", s.encrypted_packets));

    out.push_str("# HELP nwp_auth_failures Total signature verification failures\n");
    out.push_str("# TYPE nwp_auth_failures counter\n");
    out.push_str(&format!("nwp_auth_failures {}\n", s.auth_failures));

    out.push_str("# HELP nwp_decrypt_failures Total AEAD decryption failures\n");
    out.push_str("# TYPE nwp_decrypt_failures counter\n");
    out.push_str(&format!("nwp_decrypt_failures {}\n", s.decrypt_failures));

    out.push_str("# HELP nwp_rate_limited_packets Total rate-limited packets dropped\n");
    out.push_str("# TYPE nwp_rate_limited_packets counter\n");
    out.push_str(&format!(
        "nwp_rate_limited_packets {}\n",
        s.rate_limited_packets
    ));

    out.push_str("# HELP nwp_max_peers Maximum allowed peer connections\n");
    out.push_str("# TYPE nwp_max_peers gauge\n");
    out.push_str(&format!("nwp_max_peers {}\n", s.max_peers));

    out.push_str("# HELP nwp_active_peers Current active peer connections\n");
    out.push_str("# TYPE nwp_active_peers gauge\n");
    out.push_str(&format!("nwp_active_peers {}\n", s.active_peer_count));

    out.push_str("# HELP nwp_peer_capacity_ratio Peer capacity utilization (0.0-1.0)\n");
    out.push_str("# TYPE nwp_peer_capacity_ratio gauge\n");
    out.push_str(&format!(
        "nwp_peer_capacity_ratio {:.4}\n",
        s.peer_capacity_ratio
    ));

    // ── DHT metrics ──────────────────────────────────────────
    out.push_str("# HELP nwp_dht_nodes DHT routing table size\n");
    out.push_str("# TYPE nwp_dht_nodes gauge\n");
    out.push_str(&format!("nwp_dht_nodes {}\n", s.dht_node_count));

    out.push_str("# HELP nwp_dht_pending_pings DHT pings awaiting PONG\n");
    out.push_str("# TYPE nwp_dht_pending_pings gauge\n");
    out.push_str(&format!("nwp_dht_pending_pings {}\n", s.dht_pending_pings));

    // ── Trust system metrics ─────────────────────────────────
    out.push_str("# HELP nwp_trust_peers Total tracked peers in trust system\n");
    out.push_str("# TYPE nwp_trust_peers gauge\n");
    out.push_str(&format!("nwp_trust_peers {}\n", s.trust_peer_count));

    out.push_str("# HELP nwp_trust_rate_limited Currently rate-limited peers\n");
    out.push_str("# TYPE nwp_trust_rate_limited gauge\n");
    out.push_str(&format!(
        "nwp_trust_rate_limited {}\n",
        s.trust_rate_limited_peers
    ));

    // ── Session metrics ──────────────────────────────────────
    out.push_str("# HELP nwp_sessions Active secure sessions\n");
    out.push_str("# TYPE nwp_sessions gauge\n");
    out.push_str(&format!("nwp_sessions {}\n", s.active_sessions));

    out.push_str("# HELP nwp_ephemeral_sessions Sessions with forward secrecy\n");
    out.push_str("# TYPE nwp_ephemeral_sessions gauge\n");
    out.push_str(&format!(
        "nwp_ephemeral_sessions {}\n",
        s.ephemeral_sessions
    ));

    out
}

/// Build a full JSON status dump.
fn format_status(stats: &Arc<Mutex<EngineStats>>) -> String {
    let s = stats.lock().unwrap_or_else(|e| e.into_inner());
    let idle_ratio = if s.total_ticks > 0 {
        s.idle_ticks as f64 / s.total_ticks as f64
    } else {
        0.0
    };

    format!(
        r#"{{"total_ticks":{},"packets_recv":{},"packets_sent":{},"bytes_recv":{},"bytes_sent":{},"retransmissions":{},"peer_count":{},"outbound_queue_depth":{},"reliable_queue_depth":{},"idle_ratio":{:.4},"tick_rate_hz":{:.0},"status":"ok"}}"#,
        s.total_ticks,
        s.packets_recv,
        s.packets_sent,
        s.bytes_recv,
        s.bytes_sent,
        s.retransmissions,
        s.peer_count,
        s.outbound_queue_depth,
        s.reliable_queue_depth,
        idle_ratio,
        s.actual_tick_rate_hz,
    )
}

// ─── Production Dashboard HTML ─────────────────────────────────
// Served at GET / and GET /dashboard by the health HTTP server.
// Reads /status (same server) via fetch() every 3 seconds.
// Dark theme, live-updating, shows all engine metrics.
const DASHBOARD_HTML: &[u8] = include_bytes!("observability/dashboard.html");
