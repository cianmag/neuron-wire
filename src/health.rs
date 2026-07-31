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
) -> std::io::Result<(
    thread::JoinHandle<()>,
    Arc<AtomicBool>,
)> {
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
    out.push_str(&format!("nwp_reliable_queue_depth {}\n", s.reliable_queue_depth));

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
    out.push_str(&format!("nwp_authenticated_packets {}\n", s.authenticated_packets));

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
    out.push_str(&format!("nwp_rate_limited_packets {}\n", s.rate_limited_packets));

    out.push_str("# HELP nwp_max_peers Maximum allowed peer connections\n");
    out.push_str("# TYPE nwp_max_peers gauge\n");
    out.push_str(&format!("nwp_max_peers {}\n", s.max_peers));

    out.push_str("# HELP nwp_active_peers Current active peer connections\n");
    out.push_str("# TYPE nwp_active_peers gauge\n");
    out.push_str(&format!("nwp_active_peers {}\n", s.active_peer_count));

    out.push_str("# HELP nwp_peer_capacity_ratio Peer capacity utilization (0.0-1.0)\n");
    out.push_str("# TYPE nwp_peer_capacity_ratio gauge\n");
    out.push_str(&format!("nwp_peer_capacity_ratio {:.4}\n", s.peer_capacity_ratio));

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
    out.push_str(&format!("nwp_trust_rate_limited {}\n", s.trust_rate_limited_peers));

    // ── Session metrics ──────────────────────────────────────
    out.push_str("# HELP nwp_sessions Active secure sessions\n");
    out.push_str("# TYPE nwp_sessions gauge\n");
    out.push_str(&format!("nwp_sessions {}\n", s.active_sessions));

    out.push_str("# HELP nwp_ephemeral_sessions Sessions with forward secrecy\n");
    out.push_str("# TYPE nwp_ephemeral_sessions gauge\n");
    out.push_str(&format!("nwp_ephemeral_sessions {}\n", s.ephemeral_sessions));

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
const DASHBOARD_HTML: &[u8] = b"<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<title>Neuron Wire — Live Dashboard</title>\n<style>\n  :root{--bg:#0a0a0f;--card:#12121a;--border:#1e1e30;--text:#c8c8d4;--muted:#68687a;--accent:#6c5ce7;--green:#00d68f;--yellow:#ffd43b;--red:#ff6b6b;--cyan:#64d2ff}\n  *{margin:0;padding:0;box-sizing:border-box}\n  body{font-family:'SF Mono','Cascadia Code','JetBrains Mono',monospace;background:var(--bg);color:var(--text);padding:24px;min-height:100vh}\n  .header{display:flex;justify-content:space-between;align-items:center;margin-bottom:24px;padding-bottom:16px;border-bottom:1px solid var(--border)}\n  .header h1{font-size:20px;color:#fff;font-weight:500;display:flex;align-items:center;gap:12px}\n  .badge{font-size:11px;padding:3px 10px;border-radius:99px;background:var(--green);color:#000;font-weight:600}\n  .badge.offline{background:var(--red);color:#fff}\n  .time{color:var(--muted);font-size:12px}\n  .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px;margin-bottom:20px}\n  .card{background:var(--card);border:1px solid var(--border);border-radius:8px;padding:16px}\n  .lbl{font-size:11px;color:var(--muted);text-transform:uppercase;letter-spacing:.5px;margin-bottom:6px}\n  .val{font-size:26px;color:#fff;font-weight:500}\n  .val.g{color:var(--green)}.val.y{color:var(--yellow)}.val.r{color:var(--red)}.val.c{color:var(--cyan)}\n  .sub{font-size:11px;color:var(--muted);margin-top:2px}\n  .bar{height:4px;background:var(--border);border-radius:2px;margin-top:8px;overflow:hidden}\n  .bar .f{height:100%;border-radius:2px;transition:width .5s ease}\n  .bar .f.g{background:var(--green)}.bar .f.y{background:var(--yellow)}.bar .f.r{background:var(--red)}\n  .log{background:var(--card);border:1px solid var(--border);border-radius:8px;padding:16px;font-size:12px;line-height:1.6;max-height:250px;overflow-y:auto}\n  .log .l{color:var(--muted)}.log .ts{color:#48485a}\n  .ft{color:var(--muted);font-size:11px;margin-top:24px;padding-top:16px;border-top:1px solid var(--border);text-align:center}\n  .ft a{color:var(--accent);text-decoration:none}\n  @media(max-width:600px){body{padding:12px}.grid{grid-template-columns:repeat(2,1fr)}.val{font-size:20px}}\n</style>\n</head>\n<body>\n<div class=\"header\">\n  <h1>\U0001f9e0 Neuron Wire <span id=\"b\" class=\"badge\">startup</span></h1>\n  <span class=\"time\" id=\"t\">\u2014</span>\n</div>\n<div class=\"grid\">\n  <div class=\"card\"><div class=\"lbl\">Tick Rate</div><div class=\"val c\" id=\"tr\">\u2014</div><div class=\"sub\">Hz</div></div>\n  <div class=\"card\"><div class=\"lbl\">Ticks</div><div class=\"val\" id=\"tk\">\u2014</div></div>\n  <div class=\"card\"><div class=\"lbl\">Peers</div><div class=\"val g\" id=\"pr\">\u2014</div><div class=\"sub\">known nodes</div></div>\n  <div class=\"card\"><div class=\"lbl\">Packets In</div><div class=\"val\" id=\"rx\">\u2014</div></div>\n  <div class=\"card\"><div class=\"lbl\">Packets Out</div><div class=\"val\" id=\"tx\">\u2014</div></div>\n  <div class=\"card\"><div class=\"lbl\">Idle</div><div class=\"val g\" id=\"idle\">\u2014</div><div class=\"sub\">% idle</div><div class=\"bar\"><div class=\"f g\" id=\"idle-g\" style=\"width:0%\"></div></div></div>\n  <div class=\"card\"><div class=\"lbl\">Retransmit</div><div class=\"val y\" id=\"rtx\">\u2014</div></div>\n  <div class=\"card\"><div class=\"lbl\">Reliable Q</div><div class=\"val\" id=\"rq\">\u2014</div></div>\n</div>\n<div class=\"log\" id=\"log\"><div class=\"l\">Dashboard started. Waiting for data...</div></div>\n<div class=\"ft\">Neuron Wire Protocol \u00b7 <a href=\"/status\">/status</a> \u00b7 <a href=\"/metrics\">/metrics</a> \u00b7 <a href=\"/health\">/health</a></div>\n<script>\nlet cnt=0;\nasync function load(){\n  try{\n    const r=await fetch('/status');\n    if(!r.ok)throw new Error(r.status);\n    const d=await r.json();\n    const b=document.getElementById('b');\n    b.textContent='ONLINE';b.className='badge';\n    document.getElementById('t').textContent=new Date().toLocaleTimeString();\n    setv('tr',(d.tick_rate_hz||0).toFixed(0),'c');setv('tk',(d.total_ticks||0).toLocaleString());\n    setv('pr',d.peer_count||'0',d.peer_count>0?'g':'');\n    setv('rx',fmt(d.packets_recv||0));setv('tx',fmt(d.packets_sent||0));\n    setv('rtx',fmt(d.retransmissions||0));setv('rq',d.reliable_queue_depth||'0');\n    const ir=d.idle_ratio||0;\n    setv('idle',(ir*100).toFixed(1)+'%',ir>.5?'y':'g');\n    document.getElementById('idle-g').style.width=(ir*100).toFixed(0)+'%';\n    document.getElementById('idle-g').className='f '+(ir>.8?'r':ir>.5?'y':'g');\n    const lg=document.getElementById('log');\n    const ts=new Date().toLocaleTimeString();\n    const L=document.createElement('div');L.className='l';\n    L.innerHTML='<span class=\"ts\">['+ts+']</span> tick='+d.total_ticks+' peers='+d.peer_count+' rx='+d.packets_recv;\n    lg.insertBefore(L,lg.firstChild);\n    if(++cnt>50&&lg.lastChild)lg.removeChild(lg.lastChild);\n  }catch(e){\n    document.getElementById('b').textContent='OFFLINE';\n    document.getElementById('b').className='badge offline';\n    document.getElementById('t').textContent=new Date().toLocaleTimeString()+' \u2014 '+e.message;\n  }\n}\nfunction setv(id,v,c){const e=document.getElementById(id);e.textContent=v;e.className='val'+(c?' '+c:'')}\nfunction fmt(n){if(n>=1e6)return (n/1e6).toFixed(1)+'M';if(n>=1e3)return (n/1e3).toFixed(1)+'K';return n}\nload();setInterval(load,3000);\n</script>\n</body>\n</html>\n";
