//! Neuron Wire Protocol — production node binary.
//!
//! Starts an NWP node with configurable networking, identity, security, and
//! STUN NAT traversal. Designed for headless deployment on VPS / bare metal.
//!
//! # Usage
//!
//! ```text
//! cargo run --bin node [-- --config ./node-config.toml] [--identity ./identity.key]
//! ```
//!
//! # Config
//!
//! See `node-config.example.toml` for all available options.
//! The binary looks for `./node-config.toml` by default.

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use neuron_wire::engine_loop::{spawn_engine, EngineConfig};
use neuron_wire::health::spawn_health_server;
use neuron_wire::identity::{IdentityError, NodeIdentity};

// ─── VERSION ────────────────────────────────────────────────────────
const VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_NAME: &str = env!("CARGO_PKG_NAME");

// ─── CLI ─────────────────────────────────────────────────────────

struct Cli {
    config: PathBuf,
    identity: PathBuf,
}

fn parse_cli() -> Cli {
    let mut config = PathBuf::from("node-config.toml");
    let mut identity = PathBuf::from("identity.key");

    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config = args.next().map(PathBuf::from).unwrap_or_else(|| {
                    eprintln!("[NODE] WARN: --config flag followed by an empty argument");
                    config
                });
            }
            "--identity" => {
                identity = args.next().map(PathBuf::from).unwrap_or_else(|| {
                    eprintln!("[NODE] WARN: --identity flag followed by an empty argument");
                    identity
                });
            }
            "--version" | "-v" => {
                println!("{PKG_NAME} v{VERSION}");
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("[NODE] WARN: unknown argument: {arg} (use --help for usage)");
            }
        }
    }

    Cli { config, identity }
}

fn print_usage() {
    println!("{PKG_NAME} v{VERSION}");
    println!();
    println!("Usage: cargo run --bin node [-- <options>]");
    println!();
    println!("Options:");
    println!("  --config <path>      TOML config file (default: ./node-config.toml)");
    println!("  --identity <path>    Ed25519 key file  (default: ./identity.key)");
    println!("  --version, -v        Print version and exit");
    println!("  --help, -h           Print this help");
}

// ─── Config ──────────────────────────────────────────────────────

/// User-facing TOML config schema.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct NodeConfig {
    node: NodeSection,
    stun: StunSection,
    dht: DhtSection,
    transport: TransportSection,
    security: SecuritySection,
}

#[derive(serde::Deserialize)]
#[serde(default)]
struct NodeSection {
    name: String,
    bind_addr: String,
    identity_file: Option<String>,
    tick_interval_ms: u64,
    /// Maximum tracked peers. 0 = unlimited. Default: 500.
    max_peers: usize,
    /// Maximum connections from a single IP. Default: 10. 0 = unlimited.
    #[serde(default = "default_per_ip_max_peers")]
    per_ip_max_peers: usize,
}

fn default_per_ip_max_peers() -> usize {
    10
}

impl Default for NodeSection {
    fn default() -> Self {
        NodeSection {
            name: "nwp-node".to_string(),
            bind_addr: "0.0.0.0:9000".to_string(),
            identity_file: None,
            tick_interval_ms: 1,
            max_peers: 500,
            per_ip_max_peers: 10,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(default)]
struct StunSection {
    enabled: bool,
    server: String,
}

impl Default for StunSection {
    fn default() -> Self {
        StunSection {
            enabled: false,
            server: "stun.l.google.com:19302".to_string(),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(default)]
struct DhtSection {
    enabled: bool,
    bootstrap_peers: Vec<String>,
    listen_port: u16,
    /// Path to persist the DHT peer cache (binary). Peers are loaded
    /// on bootstrap and saved every maintenance cycle. Prevents peer
    /// loss on restart. Default: None (no persistence).
    peer_cache_path: Option<String>,
    /// Path to persist trust scores (binary). Prevents Sybil attackers
    /// from getting a fresh start on restart. Default: None.
    trust_cache_path: Option<String>,
    /// DNS seed domain for peer discovery (e.g. "neuron-wire.io").
    /// Resolves _dht.seeds.<domain> on bootstrap.
    seed_domain: String,
}

impl Default for DhtSection {
    fn default() -> Self {
        DhtSection {
            enabled: true,
            bootstrap_peers: Vec::new(),
            listen_port: 0,
            peer_cache_path: None,
            trust_cache_path: None,
            seed_domain: String::new(),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(default)]
struct TransportSection {
    half_life_ms: f32,
    retransmit_interval_ms: u64,
}

impl Default for TransportSection {
    fn default() -> Self {
        TransportSection {
            half_life_ms: 100.0,
            retransmit_interval_ms: 10,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(default)]
struct SecuritySection {
    sign_packets: bool,
    encrypt_payloads: bool,
}

impl Default for SecuritySection {
    fn default() -> Self {
        SecuritySection {
            sign_packets: true,
            encrypt_payloads: false,
        }
    }
}

fn load_config(path: &PathBuf) -> Result<NodeConfig, Box<dyn std::error::Error>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("[NODE] Config not found at {path:?} — using defaults");
            return Ok(NodeConfig::default());
        }
        Err(e) => return Err(Box::new(e)),
    };

    let config: NodeConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Apply 12-factor environment variable overrides on top of the
/// TOML config. This lets Docker / k8s deployments customise the
/// node without mounting a config file.
///
/// Supported vars:
///   NWP_NAME, NWP_BIND_ADDR, NWP_TICK_INTERVAL_MS,
///   NWP_STUN_ENABLED, NWP_STUN_SERVER,
///   NWP_DHT_ENABLED, NWP_BOOTSTRAP_PEERS (comma-separated),
///   NWP_SIGN_PACKETS, NWP_ENCRYPT_PAYLOADS,
///   NWP_HALF_LIFE_MS, NWP_RETRANSMIT_INTERVAL_MS,
///   NWP_HEALTH_BIND,
///   NWP_PEER_CACHE_PATH, NWP_SEED_DOMAIN
fn apply_env_overrides(cfg: &mut NodeConfig) {
    if let Ok(v) = std::env::var("NWP_NAME") {
        cfg.node.name = v;
    }
    if let Ok(v) = std::env::var("NWP_BIND_ADDR") {
        cfg.node.bind_addr = v;
    }
    if let Ok(v) = std::env::var("NWP_TICK_INTERVAL_MS") {
        if let Ok(n) = v.parse::<u64>() {
            cfg.node.tick_interval_ms = n;
        }
    }

    if let Ok(v) = std::env::var("NWP_STUN_ENABLED") {
        cfg.stun.enabled = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("NWP_STUN_SERVER") {
        cfg.stun.server = v;
    }

    if let Ok(v) = std::env::var("NWP_DHT_ENABLED") {
        cfg.dht.enabled = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("NWP_BOOTSTRAP_PEERS") {
        cfg.dht.bootstrap_peers = v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Ok(v) = std::env::var("NWP_PEER_CACHE_PATH") {
        if !v.is_empty() {
            cfg.dht.peer_cache_path = Some(v);
        }
    }
    if let Ok(v) = std::env::var("NWP_TRUST_CACHE_PATH") {
        if !v.is_empty() {
            cfg.dht.trust_cache_path = Some(v);
        }
    }
    if let Ok(v) = std::env::var("NWP_SEED_DOMAIN") {
        cfg.dht.seed_domain = v;
    }

    if let Ok(v) = std::env::var("NWP_SIGN_PACKETS") {
        cfg.security.sign_packets = v == "true" || v == "1";
    }
    if let Ok(v) = std::env::var("NWP_ENCRYPT_PAYLOADS") {
        cfg.security.encrypt_payloads = v == "true" || v == "1";
    }

    if let Ok(v) = std::env::var("NWP_HALF_LIFE_MS") {
        if let Ok(n) = v.parse::<f32>() {
            cfg.transport.half_life_ms = n;
        }
    }
    if let Ok(v) = std::env::var("NWP_RETRANSMIT_INTERVAL_MS") {
        if let Ok(n) = v.parse::<u64>() {
            cfg.transport.retransmit_interval_ms = n;
        }
    }
}

/// Validate config and emit warnings for questionable settings.
fn validate_config(cfg: &NodeConfig) {
    // Bind address parse check
    if cfg.node.bind_addr.parse::<std::net::SocketAddr>().is_err() {
        eprintln!(
            "[NODE] WARN: bind_addr {:?} looks invalid — engine may fail to bind",
            cfg.node.bind_addr
        );
    }

    // Tick interval sanity
    if cfg.node.tick_interval_ms == 0 {
        eprintln!("[NODE] WARN: tick_interval_ms=0 will busy-loop at 100% CPU");
    } else if cfg.node.tick_interval_ms > 1000 {
        eprintln!(
            "[NODE] WARN: tick_interval_ms={} is very high — network responsiveness will suffer",
            cfg.node.tick_interval_ms
        );
    }

    // Retransmit interval sanity
    if cfg.transport.retransmit_interval_ms < 1 {
        eprintln!(
            "[NODE] WARN: retransmit_interval_ms={} — clamped to 1ms",
            cfg.transport.retransmit_interval_ms
        );
    }

    // Half-life
    if cfg.transport.half_life_ms <= 0.0 {
        eprintln!("[NODE] WARN: half_life_ms <= 0 means all weights decay instantly");
    }

    // STUN enabled but default server
    if cfg.stun.enabled && cfg.stun.server == "stun.l.google.com:19302" {
        eprintln!("[NODE] INFO: using Google STUN server — consider a custom one for production");
    }

    // Security off
    if !cfg.security.sign_packets {
        eprintln!("[NODE] WARN: signing disabled — packets will NOT be authenticated!");
    }

    // DHT enabled but no bootstrap peers
    if cfg.dht.enabled && cfg.dht.bootstrap_peers.is_empty() {
        eprintln!(
            "[NODE] INFO: DHT enabled but no bootstrap peers configured — node will be isolated"
        );
    }
}

// ─── Identity ────────────────────────────────────────────────────

fn load_or_create_identity(path: &PathBuf) -> Result<NodeIdentity, IdentityError> {
    match NodeIdentity::load_from_file(path) {
        Ok(id) => {
            eprintln!("[NODE] Loaded identity from {path:?}");
            Ok(id)
        }
        Err(IdentityError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("[NODE] No identity found at {path:?} — generating new keypair");
            let id = NodeIdentity::new();
            id.save_to_file(path)?;
            eprintln!("[NODE] Saved new identity to {path:?}");
            Ok(id)
        }
        Err(e) => {
            eprintln!("[NODE] WARN: could not load identity from {path:?}: {e} — using ephemeral");
            Ok(NodeIdentity::new())
        }
    }
}

// ─── Bootstrapping ──────────────────────────────────────────────

fn resolve_peers(addrs: &[String]) -> Vec<SocketAddr> {
    let mut peers = Vec::new();
    for addr_str in addrs {
        match addr_str.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    peers.push(addr);
                } else {
                    eprintln!("[NODE] WARN: bootstrap peer {addr_str} resolved to no addresses");
                }
            }
            Err(e) => {
                eprintln!("[NODE] WARN: could not resolve bootstrap peer {addr_str}: {e}");
            }
        }
    }
    peers
}

// ─── Banner ──────────────────────────────────────────────────────

fn identity_fingerprint(id: &NodeIdentity) -> String {
    id.entity_id().0[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn print_banner(cfg: &NodeConfig, identity: &NodeIdentity) {
    let fp = identity_fingerprint(identity);

    println!("╔══════════════════════════════════════════╗");
    println!("║  {name:38} ║", name = format!("{PKG_NAME}  v{VERSION}"));
    println!("╠══════════════════════════════════════════╣");
    println!("║  Node:      {:36} ║", cfg.node.name);
    println!("║  Identity:  {:36} ║", fp);
    println!("║  Bind:      {:36} ║", cfg.node.bind_addr);
    println!(
        "║  STUN:      {:36} ║",
        if cfg.stun.enabled {
            &cfg.stun.server
        } else {
            "disabled"
        }
    );
    println!(
        "║  Signing:   {:36} ║",
        if cfg.security.sign_packets {
            "Ed25519"
        } else {
            "off"
        }
    );
    println!(
        "║  Encryption:{:36} ║",
        if cfg.security.encrypt_payloads {
            "XChaCha20-Poly1305"
        } else {
            "off"
        }
    );
    println!(
        "║  DHT:       {:36} ║",
        if cfg.dht.enabled { "Kademlia" } else { "off" }
    );
    println!(
        "║  Peers:     {:36} ║",
        if cfg.dht.bootstrap_peers.is_empty() {
            "none (isolated)".to_string()
        } else {
            format!("{} configured", cfg.dht.bootstrap_peers.len())
        }
    );
    println!("╚══════════════════════════════════════════╝");
}

// ─── Main ────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logger from env vars (NWP_LOG_LEVEL, NWP_LOG_JSON)
    neuron_wire::logger::init();

    let cli = parse_cli();
    let mut config = load_config(&cli.config)?;
    apply_env_overrides(&mut config);
    validate_config(&config);

    // ── Load identity ─────────────────────────────────────────
    let identity_path = config
        .node
        .identity_file
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or(cli.identity);
    let identity = load_or_create_identity(&identity_path)?;

    // ── Seed the engine config with the loaded identity ──────
    // Extract the 32-byte Ed25519 seed so the engine creates an
    // identical NodeIdentity internally.
    let identity_seed = identity.secret_key_bytes();

    // ── Resolve bootstrap peers ──────────────────────────────
    let local_peers = if config.dht.enabled {
        resolve_peers(&config.dht.bootstrap_peers)
    } else {
        Vec::new()
    };

    // ── Build shared stats for health endpoint ───────────────
    use neuron_wire::engine_loop::EngineStats;
    let shared_stats: Arc<Mutex<EngineStats>> = Arc::new(Mutex::new(EngineStats::default()));

    // ── Build EngineConfig with the loaded identity seed ─────
    let engine_config = EngineConfig {
        bind_addr: config.node.bind_addr.clone(),
        tick_interval_ms: config.node.tick_interval_ms,
        retransmit_interval_ticks: config.transport.retransmit_interval_ms.max(1),
        cleanup_interval_ticks: 1000,
        max_outbound_queue: 10_000,
        recv_buffer_size: 65535,
        gradient_half_life_ms: config.transport.half_life_ms,
        local_peers,
        shared_stats: Some(shared_stats.clone()), // health endpoint reads this
        freshness_config: None,
        identity_seed: Some(identity_seed), // <-- use persisted identity
        security_enabled: config.security.sign_packets,
        encrypt_payloads: config.security.encrypt_payloads,
        stun_enabled: config.stun.enabled,
        stun_server: config.stun.server.clone(),
        peer_cache_path: config.dht.peer_cache_path.clone(),
        trust_cache_path: config.dht.trust_cache_path.clone(),
        seed_domain: config.dht.seed_domain.clone(),
        max_peers: config.node.max_peers,
        heartbeat_interval_ticks: 30_000, // 30 seconds default
        per_ip_max_peers: config.node.per_ip_max_peers,
    };

    // ── Spawn health HTTP server (background thread) ─────────
    let health_bind =
        std::env::var("NWP_HEALTH_BIND").unwrap_or_else(|_| "127.0.0.1:9100".to_string());
    let (_health_handle, _health_shutdown) =
        spawn_health_server(&health_bind, shared_stats.clone())?;
    eprintln!("[NODE] Health endpoint at http://{health_bind}");

    // ── Global shutdown signal ───────────────────────────────
    let shutdown = Arc::new(AtomicBool::new(false));

    // ── Spawn observability dashboard (charts + SSE) ─────────
    let dashboard_metrics = neuron_wire::observability::MetricsRegistry::new();
    let dashboard_trace = Arc::new(Mutex::new(neuron_wire::observability::TraceCollector::new()));
    let dashboard_bind =
        std::env::var("NWP_DASHBOARD_BIND").unwrap_or_else(|_| "0.0.0.0:9090".to_string());
    let dashboard_config = neuron_wire::observability::DashboardConfig {
        listen_addr: dashboard_bind.clone(),
        html_path: None,
    };
    let _dashboard_handle = neuron_wire::observability::spawn_dashboard(
        dashboard_config,
        dashboard_metrics.clone(),
        dashboard_trace.clone(),
        shutdown.clone(),
    );
    eprintln!("[NODE] Dashboard at http://{dashboard_bind}");

    // ── Bridge: EngineStats → Dashboard gauges (1Hz poll) ────
    {
        let metrics = dashboard_metrics.clone();
        let stats = shared_stats.clone();
        std::thread::Builder::new()
            .name("nwp-stats-bridge".to_string())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(s) = stats.lock() {
                    metrics.set_peer_count(s.peer_count as u64);
                    metrics.set_auth_failures(s.auth_failures);
                    metrics.set_encrypted_packets(s.encrypted_packets);
                    metrics.set_peer_capacity(s.peer_capacity_ratio);
                    metrics.set_active_sessions(s.active_sessions as u64);
                    metrics.set_ephemeral_sessions(s.ephemeral_sessions as u64);
                    metrics.set_max_peers(s.max_peers as u64);
                    metrics.set_rate_limited_peers(s.rate_limited_packets);
                    metrics.set_dht_peers(s.dht_node_count as u64);
                    metrics.set_session_count(s.active_sessions as u64);
                }
            })
            .ok();
    }

    // ── Print banner before engine starts ────────────────────
    print_banner(&config, &identity);

    // ── Spawn engine ─────────────────────────────────────────
    let (_outbound_tx, _events_rx, handle) = spawn_engine_with_graceful_shutdown(engine_config)?;

    eprintln!("[NODE] Engine running. Press Ctrl+C to shutdown.");

    // ── Wait for engine thread to finish ─────────────────────
    handle
        .join()
        .map_err(|e| format!("engine thread panicked: {e:?}"))?;

    eprintln!("[NODE] Shutdown complete. Goodbye.");
    Ok(())
}

// ─── Platform-aware engine spawning ─────────────────────────────

#[cfg(unix)]
fn spawn_engine_with_graceful_shutdown(
    config: EngineConfig,
) -> std::io::Result<(
    std::sync::mpsc::Sender<neuron_wire::engine_loop::OutgoingPacket>,
    std::sync::mpsc::Receiver<neuron_wire::engine_loop::IngressEvent>,
    std::thread::JoinHandle<()>,
)> {
    use neuron_wire::engine_loop::spawn_engine_with_ctrlc;
    spawn_engine_with_ctrlc(config, None, None)
}

#[cfg(not(unix))]
fn spawn_engine_with_graceful_shutdown(
    config: EngineConfig,
) -> std::io::Result<(
    std::sync::mpsc::Sender<neuron_wire::engine_loop::OutgoingPacket>,
    std::sync::mpsc::Receiver<neuron_wire::engine_loop::IngressEvent>,
    std::thread::JoinHandle<()>,
)> {
    // Windows: create an atomic shutdown that we never set (user must
    // rely on process termination, or we could install a SetConsoleCtrlHandler).
    // For now, the engine runs until killed.
    let shutdown = Arc::new(AtomicBool::new(false));
    spawn_engine(config, None, shutdown, None)
}
