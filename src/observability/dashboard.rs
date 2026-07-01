//! Dashboard HTTP server — REST API, SSE live feed, and static file serving.
#![allow(missing_docs)]
//!
//! A single-threaded HTTP server that serves:
//! - `/` — the self-contained dashboard HTML
//! - `/api/snapshot` — latest metrics snapshot as JSON
//! - `/api/history` — full time-series history as JSON
//! - `/api/packets` — recent packet events as JSON
//! - `/api/peers` — per-peer RTT/latency data as JSON
//! - `/api/spans` — recent OTel spans as JSON
//! - `/api/volumes` — bandwidth volume over time (per-peer breakdown)
//! - `/metrics` — Prometheus text format
//! - `/health` — health check
//! - `/events` — SSE stream (Server-Sent Events) for live updates

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::observability::metrics::MetricsRegistry;
use crate::observability::opentelemetry::TraceCollector;
use crate::observability::prometheus;

/// Dashboard configuration.
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    /// HTTP listen address (e.g. "0.0.0.0:9090")
    pub listen_addr: String,
    /// Path to the embedded dashboard HTML (use include_str! at compile time,
    /// or a file path for development).
    pub html_path: Option<String>,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        DashboardConfig {
            listen_addr: "0.0.0.0:9090".to_string(),
            html_path: None,
        }
    }
}

/// Shared server state accessible from request handlers.
pub struct DashboardState {
    pub metrics: MetricsRegistry,
    pub trace_collector: Arc<Mutex<TraceCollector>>,
    pub shutdown: Arc<AtomicBool>,
}

/// Spawn the dashboard HTTP server in a background thread.
pub fn spawn_dashboard(
    config: DashboardConfig,
    metrics: MetricsRegistry,
    trace_collector: Arc<Mutex<TraceCollector>>,
    shutdown: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    let listen_addr = config.listen_addr.clone();

    thread::Builder::new()
        .name("nwp-dashboard".to_string())
        .spawn(move || {
            let listener = match TcpListener::bind(&listen_addr) {
                Ok(l) => {
                    eprintln!("[DASHBOARD] Listening on http://{}", listen_addr);
                    l
                }
                Err(e) => {
                    eprintln!("[DASHBOARD] Failed to bind: {}", e);
                    return;
                }
            };
            listener.set_nonblocking(true).ok();

            let state = Arc::new(DashboardState {
                metrics,
                trace_collector,
                shutdown: shutdown.clone(),
            });

            // Accept loop with 100ms polling for shutdown check
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    eprintln!("[DASHBOARD] Shutting down");
                    return;
                }

                match listener.accept() {
                    Ok((stream, _addr)) => {
                        handle_connection(stream, state.clone());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("[DASHBOARD] Accept error: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })
        .expect("failed to spawn dashboard thread")
}

/// Parse the HTTP request and dispatch to the appropriate handler.
fn handle_connection(mut stream: TcpStream, state: Arc<DashboardState>) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let path = parts[1];

    // Consume headers (read until blank line)
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header == "\r\n" {
            break;
        }
    }

    // Route based on path
    let response = match (method, path) {
        ("GET", "/") | ("GET", "/dashboard") => serve_html(),
        ("GET", "/health") => ok_response("application/json", "{\"status\":\"ok\"}\n"),
        ("GET", "/api/snapshot") => {
            let snap = state.metrics.snapshot();
            let json = serde_json::to_string_pretty(&snap).unwrap_or_default();
            ok_response("application/json", &json)
        }
        ("GET", "/api/history") => {
            let history = state.metrics.get_history();
            let json = serde_json::to_string_pretty(&history).unwrap_or_default();
            ok_response("application/json", &json)
        }
        ("GET", "/api/packets") => {
            let packets = state.metrics.get_packet_events();
            let json = serde_json::to_string_pretty(&packets).unwrap_or_default();
            ok_response("application/json", &json)
        }
        ("GET", "/api/peers") => {
            let peers = state.metrics.get_peer_latencies();
            let json = serde_json::to_string_pretty(&peers).unwrap_or_default();
            ok_response("application/json", &json)
        }
        ("GET", "/api/spans") => {
            let spans = state.trace_collector.lock()
                .map(|tc| tc.get_spans().to_vec())
                .unwrap_or_default();
            let json = serde_json::to_string_pretty(&spans).unwrap_or_default();
            ok_response("application/json", &json)
        }
        ("GET", "/metrics") => {
            let text = prometheus::format_metrics(&state.metrics);
            ok_response("text/plain; version=0.0.4", &text)
        }
        ("GET", "/events") => {
            // SSE stream — the handler will take over the connection
            return serve_sse(stream, state);
        }
        ("GET", "/api/volumes") => {
            let peers = state.metrics.get_peer_latencies();
            let volumes: Vec<serde_json::Value> = peers.iter().map(|p| {
                serde_json::json!({
                    "addr": p.addr,
                    "packets_exchanged": p.packets_exchanged,
                    "rtt_ms": p.rtt_ms,
                    "trust_score": p.trust_score,
                })
            }).collect();
            let json = serde_json::to_string_pretty(&volumes).unwrap_or_default();
            ok_response("application/json", &json)
        }
        _ => {
            let msg = format!("{{\"error\":\"not found: {}\"}}\n", path);
            (404, format!("HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", msg.len(), msg))
        }
    };

    let (_status, resp) = response;
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

type HttpResponse = (u16, String);

fn ok_response(content_type: &str, body: &str) -> HttpResponse {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        content_type,
        body.len()
    );
    (200, headers + body)
}

fn serve_html() -> HttpResponse {
    let html = include_str!("dashboard.html");
    ok_response("text/html; charset=utf-8", html)
}

/// Server-Sent Events stream — pushes a JSON snapshot every ~500ms.
fn serve_sse(mut stream: TcpStream, state: Arc<DashboardState>) {
    let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\n";
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }
    if stream.flush().is_err() {
        return;
    }

    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            return;
        }

        let snap = state.metrics.snapshot();
        let json = serde_json::to_string(&snap).unwrap_or_default();
        let msg = format!("data: {}\n\n", json);

        if stream.write_all(msg.as_bytes()).is_err() {
            return;
        }
        if stream.flush().is_err() {
            return;
        }

        thread::sleep(Duration::from_millis(500));
    }
}
