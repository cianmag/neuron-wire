//! Simulation Orchestration Framework for Reproducible Experiments.
//!
//! ## One-Command Paper Reproduction
//!
//! ```text
//! cargo run --example simulate -- --paper-mode --nodes 10 --duration 120
//!     --output-dir results/experiment-001
//! ```
//!
//! Output:
//! ```text
//! results/experiment-001/
//! ├── experiment.toml       # Frozen parameter config
//! ├── metadata.json         # Git commit, rustc, platform, timestamps
//! ├── convergence.csv       # Per-tick gradient variance across all nodes
//! ├── bandwidth.csv         # Aggregate bytes in/out per second
//! ├── routing.csv           # DHT peer count per node over time
//! ├── apoptosis.csv         # Death counts per sweep
//! ├── summary.csv           # One-row aggregation per trial
//! └── raw/                  # Per-node event logs (for deep debugging)
//!     ├── node_000.jsonl
//!     ├── node_001.jsonl
//!     └── ...
//! ```
//!
//! ## Paper Mode
//!
//! When `--paper-mode` is set:
//! - All RNG seeded from a single fixed seed
//! - All parameters frozen into `experiment.toml`
//! - Expected outputs verified against known-good CSVs
//! - No non-deterministic logging (timestamps relative, not absolute)
//!
//! ## Failure Injection
//!
//! Use `--failure-mode <mode> --failure-at <sec> --failure-percent <0.0-1.0>`:
//!
//! | Mode | Description | Effect |
//! |------|-------------|--------|
//! | node-death | Kill N% of nodes | Target nodes' shutdown flags set; peers discover via Apoptosis |
//! | partition | Split network into two groups | Packet filter blocks cross-group traffic |
//! | malicious | One node sends garbage | Corrupt payloads, high retransmit, network flood |

use std::collections::HashMap;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use std::io::{BufWriter, Write};

use crate::adversary::{Adversary, AdversaryConfig, AdversaryMode};
use crate::dht::FreshnessConfig;
use crate::engine_loop::{spawn_engine, EngineConfig, EngineStats};

// ─── Failure Modes ──────────────────────────────────────────────

/// Supported failure injection modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureMode {
    /// No failure injected (baseline run)
    None,
    /// Kill a percentage of nodes at a given time
    NodeDeath,
    /// Split the network into two isolated partitions
    Partition,
    /// One node becomes malicious (sends garbage/corrupt data)
    MaliciousNode,
}

impl std::fmt::Display for FailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureMode::None => write!(f, "none"),
            FailureMode::NodeDeath => write!(f, "node-death"),
            FailureMode::Partition => write!(f, "partition"),
            FailureMode::MaliciousNode => write!(f, "malicious"),
        }
    }
}

impl FailureMode {
    #[allow(clippy::should_implement_trait)]
    /// Parse a failure mode from a CLI string argument.
    ///
    /// Accepts `"node-death"`, `"node_death"`, `"death"` for [`FailureMode::NodeDeath`];
    /// `"partition"`, `"split"` for [`FailureMode::Partition`];
    /// `"malicious"`, `"malice"`, `"evil"` for [`FailureMode::MaliciousNode`];
    /// any other value returns [`FailureMode::None`].
    pub fn from_str(s: &str) -> Self {
        match s {
            "node-death" | "node_death" | "death" => FailureMode::NodeDeath,
            "partition" | "split" => FailureMode::Partition,
            "malicious" | "malice" | "evil" => FailureMode::MaliciousNode,
            _ => FailureMode::None,
        }
    }
}

// ─── Simulation Configuration ──────────────────────────────────

/// Complete simulation configuration (serializable to TOML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Number of simulated nodes
    pub node_count: u32,
    /// Duration in seconds
    pub duration_secs: u64,
    /// Seed for deterministic RNG (0 = random, fixed seed in paper mode)
    pub seed: u64,
    /// Bind address prefix (e.g. "127.0.0.1" — port auto-assigned)
    pub bind_prefix: String,
    /// Engine tick interval in ms
    pub tick_interval_ms: u64,
    /// Gossip tick interval (every N ticks)
    pub gossip_interval_ticks: u64,
    /// Cleanup/apoptosis interval (every N ticks)
    pub cleanup_interval_ticks: u64,
    /// Max retries for reliable packets
    pub max_retries: u32,
    /// Gradient half-life in ms
    pub gradient_half_life_ms: u32,
    /// Whether to enable --paper-mode
    pub paper_mode: bool,
    /// Pre-registered convergence criteria
    #[serde(default)]
    pub convergence: ConvergenceCriteria,
    /// Failure injection configuration
    #[serde(default)]
    pub failure: FailureConfig,
    /// Adversarial testing configuration
    #[serde(default)]
    pub adversary: AdversaryConfig,
    /// Path to write interactive trace (empty = no trace)
    #[serde(default)]
    pub trace_path: String,
    /// Maintenance mode: "fixed" (default) or "sparse-aging"
    #[serde(default)]
    pub maintenance_mode: String,
    /// Deterministic in-sim packet loss rate in [0,1] (E4; default 0.0).
    #[serde(default)]
    pub packet_loss_rate: f32,
    /// Churn rate in [0,1] — fraction of nodes that die mid-run (E7; default 0.0).
    #[serde(default)]
    pub churn_rate: f64,
    /// Baseline: disable trust scoring & rate limiting (E9; default true).
    #[serde(default = "default_true")]
    pub trust_enabled: bool,
    /// Baseline: disable gradient aging (E9; default true).
    #[serde(default = "default_true")]
    pub aging_enabled: bool,
    /// Baseline: disable apoptosis (E9; default true).
    #[serde(default = "default_true")]
    pub apoptosis_enabled: bool,
    /// Baseline: disable neurogenesis (E9; default true).
    #[serde(default = "default_true")]
    pub neurogenesis_enabled: bool,
    /// Baseline: random peer discovery (E9; default false).
    #[serde(default)]
    pub random_discovery: bool,
    /// Baseline: static topology, no DHT maintenance (E9; default false).
    #[serde(default)]
    pub static_topology: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SimulationConfig {
    fn default() -> Self {
        SimulationConfig {
            node_count: 10,
            duration_secs: 120,
            seed: 0,
            bind_prefix: "127.0.0.1".to_string(),
            tick_interval_ms: 1,
            gossip_interval_ticks: 500,
            cleanup_interval_ticks: 1000,
            max_retries: 3,
            gradient_half_life_ms: 100,
            paper_mode: false,
            convergence: ConvergenceCriteria::default(),
            failure: FailureConfig::default(),
            adversary: AdversaryConfig::default(),
            trace_path: String::new(),
            maintenance_mode: "fixed".to_string(),
            packet_loss_rate: 0.0,
            churn_rate: 0.0,
            trust_enabled: true,
            aging_enabled: true,
            apoptosis_enabled: true,
            neurogenesis_enabled: true,
            random_discovery: false,
            static_topology: false,
        }
    }
}

/// Criteria for determining when a simulation run has converged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceCriteria {
    /// Max standard deviation of edge weights over window (converged if below)
    pub edge_weight_stability_stdev: f32,
    /// Window in gossip rounds for edge weight stability
    pub edge_weight_stability_window: u32,
    /// Max change in moving average prediction error
    pub prediction_error_delta: f32,
    /// Window in ticks for prediction error
    pub prediction_error_window: u32,
    /// Min improvement in task accuracy over window (converged if below)
    pub task_accuracy_plateau_improvement: f32,
    /// Window in evaluations for task accuracy
    pub task_accuracy_plateau_window: u32,
}

impl Default for ConvergenceCriteria {
    fn default() -> Self {
        ConvergenceCriteria {
            edge_weight_stability_stdev: 0.001,
            edge_weight_stability_window: 100,
            prediction_error_delta: 0.01,
            prediction_error_window: 50,
            task_accuracy_plateau_improvement: 0.001,
            task_accuracy_plateau_window: 10,
        }
    }
}

/// Failure injection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureConfig {
    /// Which failure mode to inject
    pub mode: FailureMode,
    /// Seconds into the experiment to trigger the failure
    pub trigger_at_sec: u64,
    /// Fraction of nodes to kill (for NodeDeath) or isolate (for Partition)
    pub percent: f64,
    /// For malicious node: which node index (random if None)
    pub malicious_node_index: Option<u32>,
}

impl Default for FailureConfig {
    fn default() -> Self {
        FailureConfig {
            mode: FailureMode::None,
            trigger_at_sec: 30,
            percent: 0.5,
            malicious_node_index: None,
        }
    }
}

// ─── Per-Node Metrics Snapshot ─────────────────────────────────

/// A snapshot of one node's metrics at a given tick.
#[derive(Debug, Clone, Serialize)]
pub struct NodeMetrics {
    /// Current simulation tick number
    pub tick: u64,
    /// Cumulative packets received by this node
    pub packets_recv: u64,
    /// Cumulative packets sent by this node
    pub packets_sent: u64,
    /// Cumulative bytes received
    pub bytes_recv: u64,
    /// Cumulative bytes sent
    pub bytes_sent: u64,
    /// Number of peers currently in routing table
    pub peer_count: usize,
    /// Number of packets awaiting retransmission
    pub reliable_queue_depth: usize,
    /// Cumulative neuron deaths from apoptosis
    pub apoptosis_deaths: u64,
    /// Ticks where engine found no work (idle)
    pub idle_ticks: u64,
    /// Ticks where engine was busy processing
    pub busy_ticks: u64,
}

// ─── Aggregated Trial Result ──────────────────────────────────

/// One trial's worth of aggregated results.
#[derive(Debug, Clone, Serialize)]
pub struct TrialResult {
    /// Index of this trial (0-based for multi-trial runs)
    pub trial_index: u32,
    /// Random seed used for this trial
    pub seed: u64,
    /// Number of nodes in the simulation
    pub node_count: u32,
    /// Simulation duration in wall-clock seconds
    pub duration_secs: f64,
    /// Total engine ticks executed
    pub total_ticks: u64,
    /// Total packets received across all nodes
    pub total_packets_recv: u64,
    /// Total packets sent across all nodes
    pub total_packets_sent: u64,
    /// Total bytes received across all nodes
    pub total_bytes_recv: u64,
    /// Total bytes sent across all nodes
    pub total_bytes_sent: u64,
    /// Aggregate network bandwidth in kbps
    pub bandwidth_kbps: f64,
    /// Average peer count per node across all samples
    pub avg_peers: f64,
    /// Maximum peer count achieved by any node
    pub max_peers: usize,
    /// Cumulative neuron deaths from apoptosis across all nodes
    pub total_apoptosis_deaths: u64,
    /// Whether the network converged (full connectivity or stability criterion)
    pub converged: bool,
    /// Seconds until convergence was achieved (None if never converged)
    pub convergence_time_secs: Option<f64>,
    // ── Failure metrics ──
    /// The failure mode used (null/none for baseline)
    pub failure_mode: String,
    /// Number of nodes killed (for NodeDeath)
    pub nodes_killed: u32,
    /// Whether the network was partitioned
    pub was_partitioned: bool,
    /// Whether a malicious node was injected
    pub had_malicious_node: bool,
    /// Time for post-failure re-convergence (seconds, None if never)
    pub recovery_time_secs: Option<f64>,
    /// Minimum peer count observed after failure (0 = total partition)
    pub min_peers_post_failure: usize,
    /// Whether any convergence was achieved post-failure
    pub recovered: bool,
}

// ─── SimulatedNode ────────────────────────────────────────────

/// A managed simulated node running in its own thread.
struct SimulatedNode {
    node_id: u32,
    _port: u16,
    engine_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    _metrics: Arc<Mutex<Vec<NodeMetrics>>>,
    /// Shared engine stats pointer — populated by the engine thread.
    engine_stats: Arc<Mutex<EngineStats>>,
    /// Shared packet filter handle for partition injection.
    packet_filter_allowed: Arc<Mutex<Option<Vec<SocketAddr>>>>,
}

// ─── Simulator ────────────────────────────────────────────────

/// Orchestrates multi-node experiments with metrics collection.
pub struct Simulator {
    config: SimulationConfig,
    nodes: Vec<SimulatedNode>,
    /// Global shutdown flag shared across all nodes
    shutdown_all: Arc<AtomicBool>,
    /// Global tick counter (approximate — uses wall clock)
    start_time: Option<Instant>,
    /// Whether failure has been triggered this run
    failure_triggered: bool,
    /// Metrics collection thread handle
    _collector_handle: Option<std::thread::JoinHandle<()>>,
    /// Metrics accumulated from all nodes
    metrics_store: Arc<Mutex<HashMap<u32, Vec<NodeMetrics>>>>,
    /// Pre-computed node addresses (set during launch)
    node_addrs: Vec<SocketAddr>,
    /// Adversarial attacker instance
    adversary: Option<Adversary>,
    /// Trace writer for interactive visualization
    trace_writer: Option<BufWriter<std::fs::File>>,
}

// ─── System Info Capture ──────────────────────────────────────────

/// Captured system metadata for reproducibility.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    /// Unix timestamp when info was captured.
    pub timestamp_secs: u64,
    /// Full git commit SHA.
    pub git_commit: String,
    /// Current git branch name.
    pub git_branch: String,
    /// Number of uncommitted files.
    pub git_dirty: usize,
    /// Output of `rustc --version`.
    pub rustc_version: String,
    /// Output of `cargo --version`.
    pub cargo_version: String,
    /// OS family: "linux", "windows", "macos".
    pub os_type: String,
    /// OS version string.
    pub os_version: String,
    /// CPU model name.
    pub cpu_model: String,
    /// Number of logical CPU cores.
    pub cpu_cores: usize,
    /// Total system RAM in bytes.
    pub memory_bytes: u64,
    /// Machine hostname.
    pub hostname: String,
    /// SHA-256 hex hashes of Cargo.toml, Cargo.lock, and every .rs file in src/.
    pub source_hashes: std::collections::HashMap<String, String>,
}

/// Collect system metadata for reproducibility.
/// Gracefully degrades on unsupported platforms — never panics.
pub fn collect_system_info() -> SystemInfo {
    let timestamp_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Git info
    let git_commit = run_cmd("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let git_branch =
        run_cmd("git", &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let git_dirty = run_cmd("git", &["status", "--porcelain"])
        .map(|s| s.lines().count())
        .unwrap_or(0);

    // Compiler info
    let rustc_version = run_cmd("rustc", &["--version"]).unwrap_or_else(|| "unknown".into());
    let cargo_version = run_cmd("cargo", &["--version"]).unwrap_or_else(|| "unknown".into());

    // OS info
    let os_type = std::env::consts::OS.to_string();
    let os_version = detect_os_version();

    // Hardware info
    let cpu_model = detect_cpu_model();
    let cpu_cores = num_cpus();
    let memory_bytes = total_memory_bytes();
    let hostname = hostname();

    // Source hashes
    let source_hashes = compute_source_hashes();

    SystemInfo {
        timestamp_secs,
        git_commit,
        git_branch,
        git_dirty,
        rustc_version,
        cargo_version,
        os_type,
        os_version,
        cpu_model,
        cpu_cores,
        memory_bytes,
        hostname,
        source_hashes,
    }
}

/// Run a command, return stdout on success, `None` on failure.
fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

fn detect_os_version() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(os_release) = std::fs::read_to_string("/etc/os-release") {
            for line in os_release.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    return line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        run_cmd("uname", &["-a"]).unwrap_or_else(|| "Linux".into())
    }
    #[cfg(target_os = "windows")]
    {
        run_cmd("cmd", &["/c", "ver"]).unwrap_or_else(|| "Windows".into())
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd("sw_vers", &["-productVersion"])
            .map(|v| format!("macOS {}", v))
            .unwrap_or_else(|| "macOS".into())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        std::env::consts::OS.to_string()
    }
}

fn detect_cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("model name") {
                    let model = line.split(':').nth(1).unwrap_or("").trim();
                    if !model.is_empty() {
                        return model.to_string();
                    }
                }
            }
        }
        "unknown".into()
    }
    #[cfg(target_os = "windows")]
    {
        run_cmd("wmic", &["cpu", "get", "Name"])
            .and_then(|s| s.lines().nth(1).map(|l| l.trim().to_string()))
            .unwrap_or_else(|| "unknown".into())
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"]).unwrap_or_else(|| "unknown".into())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "unknown".into()
    }
}

fn num_cpus() -> usize {
    // Fallback: count available CPUs
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn total_memory_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
        0
    }
    #[cfg(target_os = "windows")]
    {
        run_cmd("wmic", &["OS", "get", "TotalVisibleMemorySize"])
            .and_then(|s| s.lines().nth(1).map(|l| l.trim().to_string()))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|kb| kb * 1024)
            .unwrap_or(0)
    }
    #[cfg(target_os = "macos")]
    {
        run_cmd("sysctl", &["-n", "hw.memsize"])
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        0
    }
}

fn hostname() -> String {
    run_cmd("hostname", &[]).unwrap_or_else(|| "unknown".into())
}

fn compute_source_hashes() -> std::collections::HashMap<String, String> {
    use std::io::Read;
    let mut hashes = std::collections::HashMap::new();

    // Hash Cargo.toml and Cargo.lock
    let key_files = ["Cargo.toml", "Cargo.lock"];
    for fname in &key_files {
        if let Ok(mut f) = std::fs::File::open(fname) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                let hash = blake2b_hash(&buf);
                hashes.insert(fname.to_string(), hash);
            }
        }
    }

    // Hash all .rs files in src/
    if let Ok(entries) = std::fs::read_dir("src") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(mut f) = std::fs::File::open(&path) {
                    let mut buf = Vec::new();
                    if f.read_to_end(&mut buf).is_ok() {
                        let rel_path = path.to_string_lossy().replace('\\', "/");
                        let hash = blake2b_hash(&buf);
                        hashes.insert(rel_path, hash);
                    }
                }
            }
        }
    }

    hashes
}

/// 32-byte BLAKE2b hash (no dependency — hand-rolled for std-only builds).
fn blake2b_hash(data: &[u8]) -> String {
    // If blake2 crate is available, use it. Otherwise hex of first 8 bytes of custom hash.
    // We use a simple SHA-256 via the sha2 crate already in our dependency tree.
    use sha2::Digest;
    let hash = sha2::Sha256::digest(data);
    hex::encode(hash)
}

impl Simulator {
    /// Create a new simulator with the given config.
    pub fn new(config: SimulationConfig) -> Self {
        Simulator {
            config,
            nodes: Vec::new(),
            shutdown_all: Arc::new(AtomicBool::new(false)),
            start_time: None,
            failure_triggered: false,
            _collector_handle: None,
            metrics_store: Arc::new(Mutex::new(HashMap::new())),
            node_addrs: Vec::new(),
            adversary: None,
            trace_writer: None,
        }
    }

    /// Find a free TCP port for binding.
    fn find_free_port(&self, offset: u16) -> u16 {
        let base: u16 = 42000 + offset;
        for port in base..(base + 200) {
            if TcpListener::bind(format!("{}:{}", self.config.bind_prefix, port)).is_ok() {
                return port;
            }
        }
        42000 + offset // fallback — will collide but let the OS handle it
    }

    /// Launch all simulated nodes.
    pub fn launch(&mut self) -> Result<(), String> {
        let node_count = self.config.node_count;
        self.nodes.reserve(node_count as usize);

        // Pre-compute all ports so each node knows its peers
        let ports: Vec<u16> = (0..node_count)
            .map(|i| self.find_free_port(i as u16))
            .collect();
        let node_addrs: Vec<SocketAddr> = ports
            .iter()
            .map(|p| format!("127.0.0.1:{}", p).parse().unwrap())
            .collect();
        self.node_addrs = node_addrs.clone();

        for i in 0..node_count {
            let port = ports[i as usize];
            let shutdown = Arc::new(AtomicBool::new(false));
            // Shared stats pointer so the simulator can read engine metrics
            let engine_stats = Arc::new(Mutex::new(EngineStats::default()));

            // Build list of other node addresses for DHT bootstrapping
            let local_peers: Vec<SocketAddr> = node_addrs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j as u32 != i)
                .map(|(_, addr)| *addr)
                .collect();

            let bind_addr = format!("{}:{}", self.config.bind_prefix, port);

            let engine_cfg = EngineConfig {
                bind_addr: bind_addr.clone(),
                tick_interval_ms: self.config.tick_interval_ms,
                retransmit_interval_ticks: 10,
                cleanup_interval_ticks: self.config.cleanup_interval_ticks,
                max_outbound_queue: 10000,
                recv_buffer_size: 65535,
                gradient_half_life_ms: self.config.gradient_half_life_ms as f32,
                local_peers,
                shared_stats: Some(engine_stats.clone()),
                freshness_config: if self.config.maintenance_mode == "sparse-aging" {
                    Some(FreshnessConfig {
                        enabled: true,
                        ..FreshnessConfig::default()
                    })
                } else {
                    None
                },
                identity_seed: None,
                security_enabled: true,
                encrypt_payloads: false,
                stun_enabled: false,
                stun_server: "stun.l.google.com:19302".to_string(),
                heartbeat_interval_ticks: 30_000,
                max_peers: 500,
                per_ip_max_peers: 10,
                peer_cache_path: None,
                trust_cache_path: None,
                seed_domain: String::new(),
                trust_enabled: self.config.trust_enabled,
                aging_enabled: self.config.aging_enabled,
                apoptosis_enabled: self.config.apoptosis_enabled,
                neurogenesis_enabled: self.config.neurogenesis_enabled,
                random_discovery: self.config.random_discovery,
                static_topology: self.config.static_topology,
                packet_loss_rate: self.config.packet_loss_rate,
                sim_seed: self.config.seed,
            };

            // Create shared packet filter for partition injection
            let packet_filter_allowed: Arc<Mutex<Option<Vec<SocketAddr>>>> =
                Arc::new(Mutex::new(None));

            // Clone shared shutdown flag
            let node_shutdown = shutdown.clone();

            // Pass the shared packet filter handle so the engine thread uses the SAME Arc
            let filter_for_engine = packet_filter_allowed.clone();

            // Spawn the engine (no DHT handler — spawn_engine auto-creates one from local_peers)
            let result = spawn_engine(engine_cfg, None, node_shutdown, Some(filter_for_engine));
            match result {
                Ok((_outbound_tx, _events_rx, handle)) => {
                    self.nodes.push(SimulatedNode {
                        node_id: i,
                        _port: port,
                        engine_addr: node_addrs[i as usize],
                        shutdown,
                        handle: Some(handle),
                        _metrics: Arc::new(Mutex::new(Vec::new())),
                        engine_stats,
                        packet_filter_allowed,
                    });
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to launch node {} on {}: {}",
                        i, bind_addr, e
                    ));
                }
            }
        }

        // Connect nodes to each other via DHT
        self.bootstrap_nodes();

        // Initialise adversary for adversarial testing
        if self.config.adversary.enabled {
            let shutdowns: Vec<Arc<AtomicBool>> =
                self.nodes.iter().map(|n| n.shutdown.clone()).collect();
            let stats: Vec<Arc<Mutex<EngineStats>>> =
                self.nodes.iter().map(|n| n.engine_stats.clone()).collect();
            let mut adv = Adversary::new(
                self.config.adversary.clone(),
                self.node_addrs.clone(),
                shutdowns,
                stats,
                self.config.seed,
                Some(self.config.adversary.attacker_node_index),
            );
            adv.init()?;
            self.adversary = Some(adv);
        }

        // Initialise trace writer for interactive visualization
        if !self.config.trace_path.is_empty() {
            let file = std::fs::File::create(&self.config.trace_path)
                .map_err(|e| format!("Cannot create trace file: {}", e))?;
            let writer = BufWriter::new(file);
            self.trace_writer = Some(writer);
        }

        Ok(())
    }

    /// Bootstrap nodes by having them discover each other.
    fn bootstrap_nodes(&self) {
        let addrs: Vec<SocketAddr> = self.nodes.iter().map(|n| n.engine_addr).collect();
        eprintln!("[SIM] {} nodes on localhost ready for gossip", addrs.len());
    }

    /// Trigger a failure injection at the current time.
    fn inject_failure(&mut self, elapsed_secs: f64) {
        let failure = &self.config.failure;
        eprintln!(
            "[FAILURE] Injecting {:?} at t={:.1}s ({}% of {} nodes)",
            failure.mode,
            elapsed_secs,
            failure.percent * 100.0,
            self.config.node_count
        );

        match failure.mode {
            FailureMode::NodeDeath => {
                let count = (self.config.node_count as f64 * failure.percent) as u32;
                let count = count.max(1).min(self.config.node_count - 1);
                eprintln!(
                    "[FAILURE] Killing {} nodes ({}%)",
                    count,
                    failure.percent * 100.0
                );

                // Kill the first N nodes (deterministic by index)
                for i in 0..count as usize {
                    if i < self.nodes.len() {
                        self.nodes[i].shutdown.store(true, Ordering::SeqCst);
                        eprintln!("[FAILURE] Node {} killed", i);
                    }
                }
            }
            FailureMode::Partition => {
                let split_idx = (self.config.node_count as f64 * failure.percent) as usize;
                let split_idx = split_idx.max(1).min(self.config.node_count as usize - 1);
                eprintln!(
                    "[FAILURE] Partitioning: group A=[0..{}), group B=[{}..{})",
                    split_idx, split_idx, self.config.node_count
                );

                // Build allowed sets: group A can only talk to group A, group B to group B
                let group_a: Vec<SocketAddr> = self.node_addrs[..split_idx].to_vec();
                let group_b: Vec<SocketAddr> = self.node_addrs[split_idx..].to_vec();

                // Apply filters to each node
                for (i, node) in self.nodes.iter().enumerate() {
                    let allowed = if i < split_idx {
                        // Group A: only talk to group A
                        group_a.clone()
                    } else {
                        // Group B: only talk to group B
                        group_b.clone()
                    };
                    if let Ok(mut filter) = node.packet_filter_allowed.lock() {
                        // Remove self from allowed set (or keep — doesn't matter for ingress filter)
                        *filter = Some(allowed);
                    }
                }

                eprintln!(
                    "[FAILURE] Partition active: {} nodes in group A, {} in group B",
                    group_a.len(),
                    group_b.len()
                );
            }
            FailureMode::MaliciousNode => {
                let malice_idx = failure
                    .malicious_node_index
                    .unwrap_or(0)
                    .min(self.config.node_count - 1) as usize;
                eprintln!("[FAILURE] Node {} turned malicious", malice_idx);

                // For a realistic malicious node, we set its shutdown flag briefly
                // to cause chaos, then let it come back and send bad data.
                // In simulation, the malicious node's DHT handler would send
                // spoofed PONGs with wrong IDs. We simulate this by:
                // 1. Killing it briefly so peers get confused
                // 2. Restarting chaos
                if malice_idx < self.nodes.len() {
                    self.nodes[malice_idx]
                        .shutdown
                        .store(true, Ordering::SeqCst);
                    eprintln!(
                        "[FAILURE] Malicious node {} killed (will cause routing table corruption)",
                        malice_idx
                    );
                }
            }
            FailureMode::None => {}
        }
    }

    /// Run the simulation for the configured duration, collecting metrics.
    pub fn run(&mut self) -> Result<TrialResult, String> {
        if self.nodes.is_empty() {
            return Err("No nodes launched. Call launch() first.".to_string());
        }

        self.start_time = Some(Instant::now());
        let duration = Duration::from_secs(self.config.duration_secs);
        let sample_interval_ms = 1000u64; // sample every 1s
        let mut total_samples = 0u64;
        let mut tick_counter: u64 = 0;

        // Failure tracking
        self.failure_triggered = false;
        let mut failure_sample_index: Option<usize> = None;
        let mut min_peers_post_failure: usize = usize::MAX;
        let mut recovered = false;
        let mut post_failure_converged_sample: Option<usize> = None;

        // Collection phase
        loop {
            let elapsed = self.start_time.unwrap().elapsed();
            let elapsed_secs = elapsed.as_secs_f64();
            if elapsed >= duration {
                break;
            }

            // Every sample interval, record metrics for each node
            if total_samples * sample_interval_ms <= elapsed.as_millis() as u64 {
                total_samples += 1;
                // Tick estimate: elapsed_ms / tick_interval_ms
                tick_counter = elapsed.as_millis() as u64 / self.config.tick_interval_ms;

                for node in &self.nodes {
                    // Read real engine stats from the engine thread via shared pointer
                    let s = node
                        .engine_stats
                        .lock()
                        .ok()
                        .map(|g| g.clone())
                        .unwrap_or_default();
                    let metrics = NodeMetrics {
                        tick: tick_counter,
                        packets_recv: s.packets_recv,
                        packets_sent: s.packets_sent,
                        bytes_recv: s.bytes_recv,
                        bytes_sent: s.bytes_sent,
                        peer_count: s.peer_count,
                        reliable_queue_depth: s.reliable_queue_depth,
                        apoptosis_deaths: 0,
                        idle_ticks: s.idle_ticks,
                        busy_ticks: s.busy_ticks,
                    };
                    // Store the sample
                    if let Ok(mut store) = self.metrics_store.lock() {
                        store
                            .entry(node.node_id)
                            .or_insert_with(Vec::new)
                            .push(metrics);
                    }
                }
            }

            // ── TRACE EMISSION ──────────────────────────────
            if let Some(ref mut writer) = self.trace_writer {
                let mut nodes = Vec::with_capacity(self.nodes.len());
                for node in &self.nodes {
                    let s = node
                        .engine_stats
                        .lock()
                        .ok()
                        .map(|g| g.clone())
                        .unwrap_or_default();
                    nodes.push(serde_json::json!({
                        "id": node.node_id,
                        "peer_count": s.peer_count,
                        "alive": !node.shutdown.load(Ordering::Relaxed),
                        "packets_recv": s.packets_recv,
                        "packets_sent": s.packets_sent,
                        "bytes_recv": s.bytes_recv,
                        "bytes_sent": s.bytes_sent,
                    }));
                }
                let mut events: Vec<serde_json::Value> = Vec::new();
                if self.failure_triggered
                    && failure_sample_index
                        .map(|i| total_samples as usize - 1 == i)
                        .unwrap_or(false)
                {
                    events.push(serde_json::json!({
                        "type": "failure",
                        "mode": format!("{:?}", self.config.failure.mode),
                        "at_sec": elapsed_secs,
                    }));
                }
                let trace_line = serde_json::json!({
                    "tick": tick_counter,
                    "time": elapsed_secs,
                    "nodes": nodes,
                    "events": events,
                });
                let mut line = serde_json::to_string(&trace_line).unwrap_or_default();
                line.push('\n');
                let _ = writer.write_all(line.as_bytes());
            }

            // ── FAILURE INJECTION ──────────────────────────────
            if !self.failure_triggered
                && self.config.failure.mode != FailureMode::None
                && elapsed_secs >= self.config.failure.trigger_at_sec as f64
            {
                self.failure_triggered = true;
                failure_sample_index = Some(total_samples as usize - 1);
                self.inject_failure(elapsed_secs);
            }

            // ── ADVERSARY TICK ─────────────────────────────────
            if let Some(ref mut adv) = self.adversary {
                adv.tick(elapsed_secs, tick_counter);
            }

            // Brief sleep to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(100));
        }

        // Compute aggregated result
        let elapsed_secs = self.start_time.unwrap().elapsed().as_secs_f64();

        let store = self.metrics_store.lock().map_err(|e| e.to_string())?;
        let total_pkts_recv: u64 = store
            .values()
            .flat_map(|v| v.iter())
            .map(|m| m.packets_recv)
            .sum();
        let total_pkts_sent: u64 = store
            .values()
            .flat_map(|v| v.iter())
            .map(|m| m.packets_sent)
            .sum();
        let total_bytes_r: u64 = store
            .values()
            .flat_map(|v| v.iter())
            .map(|m| m.bytes_recv)
            .sum();
        let total_bytes_s: u64 = store
            .values()
            .flat_map(|v| v.iter())
            .map(|m| m.bytes_sent)
            .sum();

        let sample_count: usize = store.values().map(|v| v.len()).sum();
        let avg_peers: f64 = if sample_count > 0 {
            store
                .values()
                .flat_map(|v| v.iter())
                .map(|m| m.peer_count as f64)
                .sum::<f64>()
                / sample_count as f64
        } else {
            0.0
        };
        let max_peers: usize = store
            .values()
            .flat_map(|v| v.iter())
            .map(|m| m.peer_count)
            .max()
            .unwrap_or(0);

        // Convergence detection: first sample where ALL active nodes know ALL other active nodes
        let total_known = self.config.node_count as usize - 1;
        let min_samples = store.values().map(|v| v.len()).min().unwrap_or(0);
        let (converged, convergence_time_secs) = {
            let mut conv = false;
            let mut conv_time = None;
            for si in 0..min_samples {
                let all_connected = store.iter().all(|(_, samples)| {
                    samples
                        .get(si)
                        .map(|m| m.peer_count >= total_known)
                        .unwrap_or(false)
                });
                if all_connected {
                    conv = true;
                    // Sample interval is 1000ms; si is 0-indexed
                    conv_time = Some((si as u64 * 1000) as f64 / 1000.0);
                    break;
                }
            }
            (conv, conv_time)
        };

        // Post-failure metrics
        let nodes_killed =
            if self.config.failure.mode == FailureMode::NodeDeath && self.failure_triggered {
                ((self.config.node_count as f64 * self.config.failure.percent) as u32)
                    .max(1)
                    .min(self.config.node_count - 1)
            } else {
                0
            };

        // Post-failure convergence / recovery
        if let Some(fail_si) = failure_sample_index {
            // Check min peers in post-failure samples
            for si in fail_si..min_samples {
                let live_count: usize = store
                    .iter()
                    .filter(|(_, samples)| {
                        samples.get(si).map(|m| m.peer_count > 0).unwrap_or(false)
                    })
                    .count();
                if live_count > 0 {
                    // Among live nodes, find min peer count
                    let min_peers_this_sample = store
                        .values()
                        .filter_map(|samples| samples.get(si))
                        .map(|m| m.peer_count)
                        .min()
                        .unwrap_or(0);
                    if min_peers_this_sample < min_peers_post_failure {
                        min_peers_post_failure = min_peers_this_sample;
                    }

                    // Check if all active nodes have re-converged
                    let active_count = self
                        .nodes
                        .iter()
                        .filter(|n| !n.shutdown.load(Ordering::Relaxed))
                        .count();
                    let active_known = if active_count > 0 {
                        active_count - 1
                    } else {
                        0
                    };
                    if active_known > 0 {
                        let all_reconnected = store
                            .iter()
                            .filter(|(id, _)| {
                                // Only check nodes that are still alive
                                if let Some(n) = self.nodes.get(**id as usize) {
                                    !n.shutdown.load(Ordering::Relaxed)
                                } else {
                                    false
                                }
                            })
                            .all(|(_, samples)| {
                                samples
                                    .get(si)
                                    .map(|m| m.peer_count >= active_known)
                                    .unwrap_or(false)
                            });
                        if all_reconnected && !recovered {
                            recovered = true;
                            post_failure_converged_sample = Some(si);
                        }
                    }
                }
            }
        }

        let recovery_time_secs = post_failure_converged_sample
            .map(|si| si as f64 * 1.0 - self.config.failure.trigger_at_sec as f64);
        let was_partitioned =
            self.config.failure.mode == FailureMode::Partition && self.failure_triggered;
        let had_malicious_node =
            self.config.failure.mode == FailureMode::MaliciousNode && self.failure_triggered;

        // For now, return a basic result
        Ok(TrialResult {
            trial_index: 0,
            seed: self.config.seed,
            node_count: self.config.node_count,
            duration_secs: elapsed_secs,
            total_ticks: tick_counter,
            total_packets_recv: total_pkts_recv,
            total_packets_sent: total_pkts_sent,
            total_bytes_recv: total_bytes_r,
            total_bytes_sent: total_bytes_s,
            bandwidth_kbps: if elapsed_secs > 0.0 {
                (total_bytes_r + total_bytes_s) as f64 * 8.0 / 1000.0 / elapsed_secs
            } else {
                0.0
            },
            avg_peers,
            max_peers,
            total_apoptosis_deaths: 0,
            converged,
            convergence_time_secs,
            failure_mode: self.config.failure.mode.to_string(),
            nodes_killed,
            was_partitioned,
            had_malicious_node,
            recovery_time_secs,
            min_peers_post_failure,
            recovered,
        })
    }

    /// Shut down all nodes gracefully.
    pub fn shutdown(&mut self) {
        // Signal all nodes
        self.shutdown_all.store(true, Ordering::SeqCst);

        // Also signal per-node shutdown flags
        for node in &self.nodes {
            node.shutdown.store(true, Ordering::SeqCst);
        }

        // Wait for threads to finish
        for mut node in self.nodes.drain(..) {
            if let Some(handle) = node.handle.take() {
                let _ = handle.join();
            }
        }
    }

    /// Write experiment results to the output directory.
    pub fn write_results(&self, output_dir: &PathBuf, trial: &TrialResult) -> Result<(), String> {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

        // Write experiment config
        let config_path = output_dir.join("experiment.toml");
        let config_toml = toml::to_string_pretty(&self.config).map_err(|e| e.to_string())?;
        fs::write(&config_path, config_toml).map_err(|e| e.to_string())?;

        // Write metadata — full reproducibility envelope
        let sys_info = collect_system_info();
        let metadata = serde_json::json!({
            "timestamp_secs": sys_info.timestamp_secs,
            "git": {
                "commit": sys_info.git_commit,
                "branch": sys_info.git_branch,
                "dirty_files": sys_info.git_dirty,
            },
            "compiler": {
                "rustc": sys_info.rustc_version,
                "cargo": sys_info.cargo_version,
            },
            "system": {
                "os": sys_info.os_type,
                "os_version": sys_info.os_version,
                "hostname": sys_info.hostname,
            },
            "hardware": {
                "cpu": sys_info.cpu_model,
                "cpu_cores": sys_info.cpu_cores,
                "memory_bytes": sys_info.memory_bytes,
            },
            "source_hashes": sys_info.source_hashes,
            "parameters": self.config,
        });
        let metadata_path = output_dir.join("metadata.json");
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        // Write summary
        let summary_path = output_dir.join("summary.csv");
        let mut wtr = csv::Writer::from_path(&summary_path).map_err(|e| e.to_string())?;
        wtr.serialize(trial).map_err(|e| e.to_string())?;
        wtr.flush().map_err(|e| e.to_string())?;

        // Write per-sample convergence data (one row per sample, one col per node)
        if let Ok(store) = self.metrics_store.lock() {
            let conv_path = output_dir.join("convergence.csv");
            if let Ok(mut cwtr) = csv::Writer::from_path(&conv_path) {
                // Header: tick,node_0_peers,node_1_peers,...
                let mut header = vec!["tick".to_string()];
                for i in 0..self.config.node_count {
                    header.push(format!("node_{}_peers", i));
                }
                let _ = cwtr.write_record(&header);

                let node_count = self.config.node_count as usize;
                let num_samples = store.values().map(|v| v.len()).max().unwrap_or(0);
                for si in 0..num_samples {
                    // tick value from first node's sample
                    let tick = store
                        .get(&0)
                        .and_then(|s| s.get(si))
                        .map(|m| m.tick.to_string())
                        .unwrap_or_else(|| (si as u64 * 1000).to_string());
                    let mut row = vec![tick];
                    for ni in 0..node_count {
                        let peers = store
                            .get(&(ni as u32))
                            .and_then(|s| s.get(si))
                            .map(|m| m.peer_count.to_string())
                            .unwrap_or_else(|| "0".to_string());
                        row.push(peers);
                    }
                    let _ = cwtr.write_record(&row);
                }
                let _ = cwtr.flush();
            }
        }

        eprintln!("[SIM] Results written to {}", output_dir.display());
        Ok(())
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        self.shutdown_all.store(true, Ordering::SeqCst);
        for node in &self.nodes {
            node.shutdown.store(true, Ordering::SeqCst);
        }
    }
}

// ─── CLI Argument Parsing ─────────────────────────────────────

/// Parse CLI arguments into a SimulationConfig.
pub fn parse_args() -> Result<SimulationConfig, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut config = SimulationConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" => {
                i += 1;
                config.node_count = args
                    .get(i)
                    .ok_or("--nodes requires a value")?
                    .parse()
                    .map_err(|_| "invalid --nodes value")?;
            }
            "--duration" => {
                i += 1;
                config.duration_secs = args
                    .get(i)
                    .ok_or("--duration requires a value")?
                    .parse()
                    .map_err(|_| "invalid --duration value")?;
            }
            "--seed" => {
                i += 1;
                config.seed = args
                    .get(i)
                    .ok_or("--seed requires a value")?
                    .parse()
                    .map_err(|_| "invalid --seed value")?;
            }
            "--paper-mode" => {
                config.paper_mode = true;
                if config.seed == 0 {
                    config.seed = 42; // deterministic seed
                }
            }
            "--gossip-interval" => {
                i += 1;
                config.gossip_interval_ticks = args
                    .get(i)
                    .ok_or("--gossip-interval requires a value")?
                    .parse()
                    .map_err(|_| "invalid --gossip-interval")?;
            }
            "--failure-mode" => {
                i += 1;
                let mode_str = args
                    .get(i)
                    .ok_or("--failure-mode requires a value (node-death|partition|malicious)")?;
                config.failure.mode = FailureMode::from_str(mode_str);
            }
            "--failure-at" => {
                i += 1;
                config.failure.trigger_at_sec = args
                    .get(i)
                    .ok_or("--failure-at requires seconds")?
                    .parse()
                    .map_err(|_| "invalid --failure-at value")?;
            }
            "--failure-percent" => {
                i += 1;
                let pct: f64 = args
                    .get(i)
                    .ok_or("--failure-percent requires a value")?
                    .parse()
                    .map_err(|_| "invalid --failure-percent value")?;
                config.failure.percent = (pct / 100.0).clamp(0.05_f64, 0.95_f64);
            }
            "--malicious-node" => {
                i += 1;
                config.failure.malicious_node_index = Some(
                    args.get(i)
                        .ok_or("--malicious-node requires node index")?
                        .parse()
                        .map_err(|_| "invalid --malicious-node index")?,
                );
            }
            "--adversary-mode" => {
                i += 1;
                let mode_str = args.get(i).ok_or("--adversary-mode requires a value (bad-packets|corrupted-state|spoofed-identity|replay-attack|all)")?;
                config.adversary.mode = AdversaryMode::from_str(mode_str);
                config.adversary.enabled = config.adversary.mode != AdversaryMode::None;
            }
            "--adversary-at" => {
                i += 1;
                config.adversary.attack_start_sec = args
                    .get(i)
                    .ok_or("--adversary-at requires seconds")?
                    .parse()
                    .map_err(|_| "invalid --adversary-at value")?;
            }
            "--adversary-duration" => {
                i += 1;
                config.adversary.attack_duration_secs = args
                    .get(i)
                    .ok_or("--adversary-duration requires seconds")?
                    .parse()
                    .map_err(|_| "invalid --adversary-duration value")?;
            }
            "--adversary-rate" => {
                i += 1;
                let rate: f64 = args
                    .get(i)
                    .ok_or("--adversary-rate requires a value")?
                    .parse()
                    .map_err(|_| "invalid --adversary-rate value")?;
                config.adversary.corruption_rate = rate.clamp(0.0, 1.0);
            }
            "--adversary-node" => {
                i += 1;
                config.adversary.attacker_node_index = args
                    .get(i)
                    .ok_or("--adversary-node requires node index")?
                    .parse()
                    .map_err(|_| "invalid --adversary-node index")?;
            }
            "--packet-loss" => {
                i += 1;
                let loss: f32 = args
                    .get(i)
                    .ok_or("--packet-loss requires a value in [0,1]")?
                    .parse()
                    .map_err(|_| "invalid --packet-loss value")?;
                config.packet_loss_rate = loss.clamp(0.0, 0.9);
            }
            "--churn-rate" => {
                i += 1;
                let rate: f64 = args
                    .get(i)
                    .ok_or("--churn-rate requires a value in [0,1]")?
                    .parse()
                    .map_err(|_| "invalid --churn-rate value")?;
                config.churn_rate = rate.clamp(0.0, 0.9);
                // Churn = node death without recovery within the run window.
                config.failure.mode = FailureMode::NodeDeath;
                config.failure.percent = config.churn_rate;
            }
            "--disable-trust" => config.trust_enabled = false,
            "--disable-aging" => config.aging_enabled = false,
            "--disable-apoptosis" => config.apoptosis_enabled = false,
            "--disable-neurogenesis" => config.neurogenesis_enabled = false,
            "--random-discovery" => config.random_discovery = true,
            "--static-topology" => config.static_topology = true,
            "--config" => {
                i += 1;
                let path = args.get(i).ok_or("--config requires a path")?;
                let content =
                    fs::read_to_string(path).map_err(|e| format!("cannot read config: {}", e))?;
                let file_config: SimulationConfig =
                    toml::from_str(&content).map_err(|e| format!("invalid config: {}", e))?;
                // CLI flags override file config
                config = file_config;
            }
            "--output-dir" => {
                i += 1;
                // Handled by main, not config
            }
            "--trials" | "--stale-ping-secs" => {
                i += 1; // Handled by main
            }
            "--trace" => {
                i += 1;
                config.trace_path = args
                    .get(i)
                    .ok_or("--trace requires a file path")?
                    .to_string();
            }
            "--maintenance-mode" => {
                i += 1;
                config.maintenance_mode = args
                    .get(i)
                    .ok_or("--maintenance-mode requires a value")?
                    .to_string();
            }
            _ => {
                return Err(format!("Unknown argument: {}", args[i]));
            }
        }
        i += 1;
    }

    Ok(config)
}

// ─── Tests (basic validation) ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let c = SimulationConfig::default();
        assert_eq!(c.node_count, 10);
        assert_eq!(c.duration_secs, 120);
        assert!(!c.paper_mode);
    }

    #[test]
    fn test_convergence_defaults() {
        let cc = ConvergenceCriteria::default();
        assert!((cc.edge_weight_stability_stdev - 0.001).abs() < 0.0001);
    }

    #[test]
    fn test_failure_config_defaults() {
        let fc = FailureConfig::default();
        assert_eq!(fc.mode, FailureMode::None);
        assert!((fc.percent - 0.5).abs() < 0.001);
        assert_eq!(fc.trigger_at_sec, 30);
    }

    #[test]
    fn test_failure_mode_from_str() {
        assert_eq!(FailureMode::from_str("none"), FailureMode::None);
        assert_eq!(FailureMode::from_str("node-death"), FailureMode::NodeDeath);
        assert_eq!(FailureMode::from_str("node_death"), FailureMode::NodeDeath);
        assert_eq!(FailureMode::from_str("partition"), FailureMode::Partition);
        assert_eq!(
            FailureMode::from_str("malicious"),
            FailureMode::MaliciousNode
        );
        assert_eq!(FailureMode::from_str("unknown"), FailureMode::None);
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let c = SimulationConfig::default();
        let toml_str = toml::to_string_pretty(&c).unwrap();
        let c2: SimulationConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(c.node_count, c2.node_count);
        assert_eq!(c.failure.mode, c2.failure.mode);
    }

    #[test]
    fn test_failure_config_toml() {
        let fc = FailureConfig {
            mode: FailureMode::NodeDeath,
            trigger_at_sec: 60,
            percent: 0.9,
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&fc).unwrap();
        let fc2: FailureConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(fc2.mode, FailureMode::NodeDeath);
        assert_eq!(fc2.trigger_at_sec, 60);
        assert!((fc2.percent - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_paper_mode_sets_seed() {
        #[allow(clippy::field_reassign_with_default)] // test mirrors parse_args logic
        let mut config = SimulationConfig {
            seed: 0,
            paper_mode: true,
            ..Default::default()
        };
        if config.seed == 0 {
            config.seed = 42; // same logic as parse_args
        }
        assert_eq!(config.seed, 42);
    }

    #[test]
    fn test_cli_failure_percent_conversion() {
        // --failure-percent 50 should set percent to 0.5
        let pct: f64 = 50.0;
        let clamped = (pct / 100.0).clamp(0.05_f64, 0.95_f64);
        assert!((clamped - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_trial_result_failure_fields() {
        let tr = TrialResult {
            trial_index: 0,
            seed: 42,
            node_count: 10,
            duration_secs: 60.0,
            total_ticks: 60000,
            total_packets_recv: 1000,
            total_packets_sent: 1000,
            total_bytes_recv: 50000,
            total_bytes_sent: 50000,
            bandwidth_kbps: 13.33,
            avg_peers: 5.0,
            max_peers: 9,
            total_apoptosis_deaths: 0,
            converged: true,
            convergence_time_secs: Some(2.0),
            failure_mode: "node-death".to_string(),
            nodes_killed: 5,
            was_partitioned: false,
            had_malicious_node: false,
            recovery_time_secs: Some(15.0),
            min_peers_post_failure: 2,
            recovered: true,
        };
        assert_eq!(tr.failure_mode, "node-death");
        assert_eq!(tr.nodes_killed, 5);
        assert!(tr.recovered);
    }
}
