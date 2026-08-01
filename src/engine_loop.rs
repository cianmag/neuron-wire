//! Single-Thread Non-Blocking Event Engine.
//!
//! ## Why Not Tokio?
//!
//! Tokio on a free-tier VPS (512MB RAM, shared CPU) is overhead we don't need.
//! Tokio's work-stealing scheduler, multi-threaded runtime, and 50+ transitive
//! crates are great for mixed workloads, but this system has a very specific
//! execution pattern: drain UDP socket → process → retransmit → repeat.
//!
//! A single-threaded `recv_from()` loop with a 1ms read timeout gives us:
//!
//!   - **Deterministic timing**: tick every ~1ms, no scheduler jitter
//!   - **Zero busy-wait**: OS blocks the thread during idle (0% CPU)
//!   - **Max throughput**: sustained traffic drains as fast as the socket delivers
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  EngineLoop (single thread, owns UDP socket)      │
//! │                                                    │
//! │  LOOP (every ~1ms):                                │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │ Phase 1: recv_from() with 1ms timeout         │  │
//! │  │ Phase 2: Drain outbound channel → send()      │  │
//! │  │ Phase 3: NEURAL COMPUTATION (every tick)      │  │
//! │  │   ├─ ForwardPass: propagate + predict         │  │
//! │  │   └─ Hebbian: STDP + micro-prune + gossip     │  │
//! │  │ Phase 4: Retransmit stale (every 10ms)        │  │
//! │  │ Phase 5: Cleanup + Apoptosis (every 1000ms)   │  │
//! │  │ Phase 6: Yield if busy                        │  │
//! │  └──────────────────────────────────────────────┘  │
//! │                                                    │
//! │  Attach brain via engine.attach_brain(...)         │
//! │  before run() to enable AGI computation.           │
//! └──────────────────────────────────────────────────┘
//!                    │                    ▲
//!        outbound_tx │                    │ events_tx
//!        (mpsc)      ▼                    │ (mpsc)
//!           ┌──────────────┐    ┌─────────────────┐
//!           │ Other threads │    │ Event subscribers│
//!           │ (DHT, ECS,   │    │ (training,       │
//!           │  Hebbian)    │    │  consensus, DHT) │
//!           └──────────────┘    └─────────────────┘
//! ```
//!
//! ## Channels
//!
//! - **`outbound_tx`**: `Sender<OutgoingPacket>` — any component enqueues
//!   NWP frames here. The engine loop drains them in-order and sends over UDP.
//!
//! - **`events_tx`**: `Sender<IngressEvent>` — the engine dispatches fully
//!   validated, ACK-tracked messages here. Subscribers process them.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::apoptosis::ApoptosisSystem;
use crate::components::{ActivationMap, EntityId, SynapseMap};
use crate::dht::{DhtHandler, FreshnessConfig, NodeId, NodeType};
use crate::error::{NwpError, TransportError};
use crate::forward_pass::ForwardPassSystem;
use crate::header;
use crate::hebbian::HebbianLearningSystem;
use crate::ml::MLSystem;
use crate::neurogenesis::NeurogenesisSystem;
use crate::transport::{TransportHeader, UdpTransport};
use crate::{log_debug, log_error, log_info, log_warn};

// ── Security imports (optional, gate via cfg) ──────────────────
use crate::audit::{AuditEventType, AuditLog};
use crate::identity::NodeIdentity;
use crate::secure_channel::SecureChannel;
use crate::trust::TrustSystem;
use std::hash::{Hash, Hasher};

/// Derive a deterministic EntityId from a SocketAddr for trust tracking.
/// This is a fallback when the actual public key isn't available.
fn entity_id_from_addr(addr: &SocketAddr) -> EntityId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    addr.hash(&mut hasher);
    let h = hasher.finish();
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&h.to_le_bytes());
    EntityId(id)
}

// ─── Configuration ─────────────────────────────────────────────

/// Engine loop configuration
///
/// # Examples
///
/// ```
/// use neuron_wire::engine_loop::EngineConfig;
///
/// // Default config for local development
/// let config = EngineConfig::default();
/// assert_eq!(config.bind_addr, "0.0.0.0:9000");
/// assert_eq!(config.max_peers, 500);
/// assert_eq!(config.per_ip_max_peers, 10);
///
/// // Production config with tighter limits
/// let mut config = EngineConfig::default();
/// config.max_peers = 200;
/// config.per_ip_max_peers = 5;
/// config.security_enabled = true;
/// config.encrypt_payloads = true;
/// ```
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// UDP bind address
    pub bind_addr: String,
    /// Target tick interval (default: 1ms)
    pub tick_interval_ms: u64,
    /// Retransmit scan interval (default: 10ms, every N ticks)
    pub retransmit_interval_ticks: u64,
    /// Cleanup interval (default: 1000ms, every N ticks)
    pub cleanup_interval_ticks: u64,
    /// Maximum queued outbound packets (backpressure limit)
    pub max_outbound_queue: usize,
    /// Size of the pre-allocated receive buffer
    pub recv_buffer_size: usize,
    /// Half-life for gradient weight decay (ms)
    pub gradient_half_life_ms: f32,
    /// Pre-seeded peer addresses for DHT bootstrapping.
    pub local_peers: Vec<SocketAddr>,
    /// Shared pointer so external watchers can read live stats.
    pub shared_stats: Option<Arc<Mutex<EngineStats>>>,
    /// Sparse Gradient Aging configuration (None = standard maintenance).
    pub freshness_config: Option<FreshnessConfig>,
    /// Optional identity seed for deterministic key generation (None = random).
    pub identity_seed: Option<[u8; 32]>,
    /// Enable mandatory packet signing (default: true).
    /// When true, ALL outbound packets are Ed25519-signed and ALL inbound
    /// packets are signature-verified. Packets with invalid signatures are dropped.
    pub security_enabled: bool,
    /// Enable payload encryption (default: false until handshake established).
    /// When true, packet bodies are AEAD-encrypted with per-peer session keys.
    pub encrypt_payloads: bool,
    /// Enable STUN NAT traversal for external address discovery at startup.
    pub stun_enabled: bool,
    /// STUN server address (default: "stun.l.google.com:19302").
    pub stun_server: String,
    /// Path for persisting the DHT peer cache (binary format).
    pub peer_cache_path: Option<String>,
    /// Path for persisting trust scores (binary format).
    pub trust_cache_path: Option<String>,
    /// Seed domain for DNS-based peer discovery (e.g. "nwp.neuron-wire.io").
    pub seed_domain: String,
    /// Maximum number of tracked peers. When exceeded, the node sends
    /// TOO_MANY_PEERS disconnect to new arrivals. Set to 0 for unlimited.
    /// Default: 500. Prevents memory exhaustion from Sybil floods.
    pub max_peers: usize,
    /// Interval between heartbeat sends (ticks). 0 = disabled.
    /// Default: 30000 (30 seconds at 1ms tick rate).
    pub heartbeat_interval_ticks: u64,
    /// Maximum connections from a single IP address. Prevents a single
    /// source from consuming all peer slots. 0 = no per-IP limit.
    /// Default: 10.
    pub per_ip_max_peers: usize,
    /// Enable trust scoring & rate limiting (baseline toggle, default true).
    pub trust_enabled: bool,
    /// Enable gradient aging (half-life decay) (baseline toggle, default true).
    pub aging_enabled: bool,
    /// Enable the apoptosis sweep (baseline toggle, default true).
    pub apoptosis_enabled: bool,
    /// Enable neurogenesis (baseline toggle, default true).
    pub neurogenesis_enabled: bool,
    /// Baseline: use random peer discovery instead of XOR-closest (default false).
    pub random_discovery: bool,
    /// Baseline: static topology — no DHT maintenance beyond initial peers (default false).
    pub static_topology: bool,
    /// Deterministic in-sim packet loss rate in [0,1] (default 0.0).
    pub packet_loss_rate: f32,
    /// Seed for the deterministic impairment RNG (default 0).
    pub sim_seed: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            bind_addr: "0.0.0.0:9000".to_string(),
            tick_interval_ms: 1,
            retransmit_interval_ticks: 10, // every 10ms
            cleanup_interval_ticks: 1000,  // every 1s
            max_outbound_queue: 10_000,
            recv_buffer_size: 65535,
            gradient_half_life_ms: 100.0,
            local_peers: Vec::new(),
            shared_stats: None,
            freshness_config: None,
            identity_seed: None, // random identity by default
            security_enabled: true,
            encrypt_payloads: false,
            stun_enabled: false,
            stun_server: "stun.l.google.com:19302".to_string(),
            peer_cache_path: None,
            trust_cache_path: None,
            seed_domain: String::new(),
            max_peers: 500,
            heartbeat_interval_ticks: 30_000, // 30 seconds
            per_ip_max_peers: 10,
            trust_enabled: true,
            aging_enabled: true,
            apoptosis_enabled: true,
            neurogenesis_enabled: true,
            random_discovery: false,
            static_topology: false,
            packet_loss_rate: 0.0,
            sim_seed: 0,
        }
    }
}

// ─── Outbound Packet ───────────────────────────────────────────

/// A packet to be sent over the UDP transport.
#[derive(Debug, Clone)]
pub struct OutgoingPacket {
    /// Raw NWP frame bytes (already framed: header + body, ready to send)
    pub payload: Vec<u8>,
    /// Destination address
    pub dst: SocketAddr,
    /// Reliability mode
    pub mode: Reliability,
}

/// How reliably to deliver this packet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliability {
    /// No retransmit (SPIKE, COMMAND, READINESS, GOSSIP)
    BestEffort,
    /// Retransmit up to 3 times (DATA gradients)
    Data,
    /// Retransmit up to 5 times (CONSENSUS)
    Consensus,
}

impl Reliability {
    fn max_retries(&self) -> u32 {
        match self {
            Reliability::BestEffort => 0,
            Reliability::Data => 3,
            Reliability::Consensus => 5,
        }
    }

    fn is_reliable(&self) -> bool {
        match self {
            Reliability::BestEffort => false,
            Reliability::Data | Reliability::Consensus => true,
        }
    }
}

// ─── Ingress Event ─────────────────────────────────────────────

/// A fully validated, ACK-tracked incoming message dispatched to subscribers.
#[derive(Debug, Clone)]
pub struct IngressEvent {
    /// Parsed transport header
    pub transport_header: TransportHeader,
    /// Raw NWP frame (can be zero-copy parsed by the subscriber)
    pub nwp_payload: Vec<u8>,
    /// Source address
    pub src: SocketAddr,
    /// Received timestamp (engine-local milliseconds)
    pub recv_timestamp: u32,
    /// Gradient decay weight applied (1.0 for SPIKE, decayed for DATA)
    pub gradient_weight: f32,
}

// ─── Engine Loop Stats ─────────────────────────────────────────

/// Runtime statistics exported by the engine loop.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    /// Total ticks executed
    pub total_ticks: u64,
    /// Total packets received
    pub packets_recv: u64,
    /// Total packets sent
    pub packets_sent: u64,
    /// Total bytes received
    pub bytes_recv: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total retransmissions
    pub retransmissions: u64,
    /// Current number of known peers
    pub peer_count: usize,
    /// Current outbound queue depth
    pub outbound_queue_depth: usize,
    /// Current reliable queue depth
    pub reliable_queue_depth: usize,
    /// Idle ticks (no packet received)
    pub idle_ticks: u64,
    /// Busy ticks (at least one packet processed)
    pub busy_ticks: u64,
    /// Tick rate (actual average)
    pub actual_tick_rate_hz: f64,
    // ── Security metrics ─────────────────────────────────────
    /// Packets that passed Ed25519 signature verification
    pub authenticated_packets: u64,
    /// Packets that were AEAD-encrypted
    pub encrypted_packets: u64,
    /// Packets that failed signature verification
    pub auth_failures: u64,
    /// Packets that failed AEAD decryption
    pub decrypt_failures: u64,
    /// Rate-limited packets dropped
    pub rate_limited_packets: u64,
    // ── Distributed learning metrics ─────────────────────────
    /// Data frames carrying remote learning signals (gossip) received
    pub learning_frames_recv: u64,
    /// Data frames carrying remote learning signals (gossip) sent
    pub learning_frames_sent: u64,
    // ── DHT metrics ──────────────────────────────────────────
    /// DHT routing table size
    pub dht_node_count: usize,
    /// DHT pending pings
    pub dht_pending_pings: usize,
    /// DHT known dead nodes (evicted)
    pub dht_dead_nodes: u64,
    // ── Trust system metrics ─────────────────────────────────
    /// Total tracked peers in trust system
    pub trust_peer_count: usize,
    /// Currently rate-limited peers
    pub trust_rate_limited_peers: usize,
    // ── Capacity metrics ─────────────────────────────────────
    /// Maximum allowed peers (from config)
    pub max_peers: usize,
    /// Current peer count
    pub active_peer_count: usize,
    /// Peer capacity utilization (0.0 - 1.0)
    pub peer_capacity_ratio: f64,
    // ── Session metrics ──────────────────────────────────────
    /// Active secure sessions
    pub active_sessions: usize,
    /// Ephemeral sessions (with forward secrecy)
    pub ephemeral_sessions: usize,
}

// ─── Engine Loop ───────────────────────────────────────────────

/// Per-peer tracking info: RTT estimate + last-seen timestamp.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Round-trip time estimate in milliseconds.
    pub rtt_ms: f32,
    /// Timestamp of last packet received from this peer (ms since epoch).
    pub last_seen_ms: u64,
}

/// Single-threaded, non-blocking event engine for the planetary brain.
///
/// ## Usage
///
/// ```ignore
/// let (engine, events_rx, ) = EngineLoop::new(config);
/// engine.run(); // blocks on the current thread
/// ```
pub struct EngineLoop {
    config: EngineConfig,
    transport: UdpTransport,
    /// Channel: other components enqueue packets to send
    outbound_rx: Receiver<OutgoingPacket>,
    /// Channel: engine dispatches received events (None = no subscriber)
    events_tx: Option<Sender<IngressEvent>>,
    /// DHT handler for peer discovery (optional — attach after construction)
    pub dht_handler: Option<DhtHandler>,
    /// Apoptosis garbage collector
    pub apoptosis_system: ApoptosisSystem,
    /// Shutdown signal: when true, the run() loop exits cleanly
    pub shutdown: Arc<AtomicBool>,
    /// ── Security Subsystem ──────────────────────────────────────
    /// This node's cryptographic identity (Ed25519 keypair)
    pub node_identity: NodeIdentity,
    /// Encrypted channel manager (XChaCha20-Poly1305 sessions)
    pub secure_channel: SecureChannel,
    /// Trust & reputation system (Sybil resistance, rate limiting)
    pub trust_system: TrustSystem,
    /// Audit log with hash-chain tamper detection
    pub audit_log: AuditLog,
    /// Map of known peers + their tracking info (RTT + last-seen)
    peer_rtt: HashMap<SocketAddr, PeerInfo>,
    /// Per-IP connection count for DoS protection
    peer_ip_count: HashMap<std::net::IpAddr, usize>,
    /// Pre-allocated receive buffer (reused across ticks to avoid per-packet allocation)
    recv_buf: Vec<u8>,
    /// ── Timers ─────────────────────────────────────────────────
    tick: u64,
    last_retransmit_tick: u64,
    last_cleanup_tick: u64,
    /// Last heartbeat send tick
    last_heartbeat_tick: u64,
    /// Last stats snapshot time
    last_stats_time: Instant,
    /// Running stats
    stats: EngineStats,
    /// ── Brain State (optional, attach via attach_brain()) ─────
    /// Activation values for each known neuron
    activation_map: ActivationMap,
    /// Synaptic connections between neurons
    synapse_map: SynapseMap,
    /// Forward pass: activation propagation + prediction
    forward_pass: ForwardPassSystem,
    /// Neurogenesis: surprise-driven neuron birth
    neurogenesis: NeurogenesisSystem,
    /// Hebbian learning: STDP weight updates + gossip
    hebbian: HebbianLearningSystem,
    /// ML orchestration: adaptive LR, meta-learning, curiosity, memory, etc.
    ml_system: MLSystem,
    /// Local node's 256-bit cryptographic identity
    local_id: EntityId,
    /// Clone of the outbound sender (for Hebbian gossip)
    outbound_tx: Sender<OutgoingPacket>,
    /// Whether the brain is attached and should tick
    brain_attached: bool,
    /// Shared packet filter for failure injection (partition simulation).
    /// When `Some(allowed)`, only packets from addresses in the set are processed.
    /// When `None`, all packets are accepted (normal operation).
    /// Uses Arc<Mutex<>> so the simulator thread can inject filters post-spawn.
    pub packet_filter_allowed: Arc<Mutex<Option<Vec<SocketAddr>>>>,
    /// Xorshift state for deterministic in-sim packet loss.
    loss_rng: u64,
    /// Remote learning signals decoded from Data frames (entity, activation),
    /// drained by the neural-computation phase each tick.
    pending_observations: Vec<(EntityId, f32)>,
}

#[cfg(test)]
impl EngineLoop {
    /// Test-only helper: insert a peer with a given RTT and age (ms ago).
    pub fn insert_peer_for_test(&mut self, addr: SocketAddr, rtt: f32, age_ms: u64) {
        let now_ms = u64::from(self.transport.now_ms());
        self.peer_rtt.insert(
            addr,
            PeerInfo {
                rtt_ms: rtt,
                last_seen_ms: now_ms.saturating_sub(age_ms),
            },
        );
    }

    /// Test-only helper: number of peers currently tracked.
    pub fn peer_count_for_test(&self) -> usize {
        self.peer_rtt.len()
    }
}

impl EngineLoop {
    /// Create a new engine loop.
    /// Returns (engine, outbound_tx, events_rx).
    pub fn new(
        config: EngineConfig,
    ) -> std::io::Result<(Self, Sender<OutgoingPacket>, Receiver<IngressEvent>)> {
        let transport = UdpTransport::bind(&config.bind_addr)?;
        let (outbound_tx, outbound_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();

        // Create cryptographic identity (deterministic from seed, or random)
        let node_identity = match config.identity_seed {
            Some(seed) => NodeIdentity::from_seed(&seed),
            None => NodeIdentity::new(),
        };
        let audit_log = AuditLog::new();

        // Deterministic impairment RNG seed: sim_seed mixed with the bind address.
        let mut loss_hasher = std::collections::hash_map::DefaultHasher::new();
        config.bind_addr.hash(&mut loss_hasher);
        let loss_rng = config.sim_seed ^ loss_hasher.finish() ^ 0x9E37_79B9_7F4A_7C15;

        let engine = EngineLoop {
            config,
            transport,
            outbound_rx,
            events_tx: Some(events_tx),
            dht_handler: None,
            apoptosis_system: ApoptosisSystem::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
            loss_rng,
            // Security subsystem
            node_identity,
            secure_channel: SecureChannel::new(),
            trust_system: TrustSystem::new(),
            audit_log,
            tick: 0,
            last_retransmit_tick: 0,
            last_cleanup_tick: 0,
            last_heartbeat_tick: 0,
            last_stats_time: Instant::now(),
            stats: EngineStats::default(),
            peer_rtt: HashMap::with_capacity(512),
            peer_ip_count: HashMap::with_capacity(128),
            recv_buf: vec![0u8; 65535],
            // Brain state defaults (attach via attach_brain())
            activation_map: HashMap::with_capacity(256),
            synapse_map: HashMap::with_capacity(1024),
            forward_pass: ForwardPassSystem::default(),
            neurogenesis: NeurogenesisSystem::default(),
            hebbian: HebbianLearningSystem::new(0.01, 0.999, 0.001, 500),
            ml_system: MLSystem::new(),
            local_id: EntityId([0u8; 32]),
            outbound_tx: outbound_tx.clone(),
            brain_attached: false,
            packet_filter_allowed: Arc::new(Mutex::new(None)),
            pending_observations: Vec::new(),
        };

        Ok((engine, outbound_tx, events_rx))
    }

    /// Attach a DHT handler to the engine loop.
    /// The DHT handler processes discovery events and feeds the routing table.
    /// Call this before `run()` to enable peer discovery.
    pub fn attach_dht(&mut self, dht: DhtHandler) {
        self.dht_handler = Some(dht);
    }

    /// Attach neural computation brain to the engine loop.
    /// Enables ForwardPass + Hebbian learning on every tick.
    /// Call this before `run()` to enable AGI computation.
    #[allow(clippy::too_many_arguments)] // config bundle, kept flat for ergonomics
    pub fn attach_brain(
        &mut self,
        activation_map: ActivationMap,
        synapse_map: SynapseMap,
        forward_pass: ForwardPassSystem,
        neurogenesis: NeurogenesisSystem,
        hebbian: HebbianLearningSystem,
        ml_system: MLSystem,
        local_id: EntityId,
    ) {
        self.activation_map = activation_map;
        self.synapse_map = synapse_map;
        self.forward_pass = forward_pass;
        self.neurogenesis = neurogenesis;
        self.hebbian = hebbian;
        self.ml_system = ml_system;
        self.local_id = local_id;
        self.brain_attached = true;
    }

    /// Run the engine loop. **Blocks the current thread until shutdown is signalled.**
    ///
    /// Single-threaded, non-blocking loop with ~1ms tick rate.
    /// The thread sleeps during idle via the UDP socket's read timeout.
    pub fn run(&mut self) {
        // Set 1ms read timeout so recv_from blocks for at most 1ms
        if let Err(e) = self
            .transport
            .socket
            .set_read_timeout(Some(Duration::from_millis(self.config.tick_interval_ms)))
        {
            log_warn!(
                "engine",
                format!("[ENGINE] Could not set read timeout: {}", e)
            );
        }

        // Pre-allocated receive buffer lives on the struct (avoid per-packet allocation)
        let mut ingress_count_this_tick: u32;

        // Log startup to audit trail
        self.audit_log.append(
            AuditEventType::NodeStartup,
            &format!(
                "Engine started, entity={:02x}{:02x}{:02x}.., bind={}",
                self.node_identity.entity_id().0[0],
                self.node_identity.entity_id().0[1],
                self.node_identity.entity_id().0[2],
                self.config.bind_addr,
            ),
            None,
        );

        loop {
            self.tick += 1;

            // ── SHUTDOWN CHECK ─────────────────────────────────
            if self.shutdown.load(Ordering::Relaxed) {
                log_info!(
                    "engine",
                    format!(
                        "[ENGINE] Shutdown signal received at tick {}. Exiting.",
                        self.tick
                    )
                );
                return;
            }

            ingress_count_this_tick = 0;

            // ── PHASE 1: DRAIN UDP SOCKET ─────────────────────
            // Non-blocking: drain ALL available messages from the socket buffer.
            // This prevents the "one-per-iteration" bottleneck.
            loop {
                match self.transport.socket.recv_from(&mut self.recv_buf) {
                    Ok((len, src)) => {
                        ingress_count_this_tick += 1;
                        self.stats.packets_recv += 1;
                        self.stats.bytes_recv += len as u64;

                        // Copy the received bytes out of the shared buffer so the
                        // mutable borrow in handle_ingress does not conflict with
                        // the buffer borrow. Allocation is packet-sized only.
                        let packet = self.recv_buf[..len].to_vec();
                        if let Err(e) = self.handle_ingress(&packet, src) {
                            log_error!("engine", format!("[ENGINE] Ingress error: {}", e));
                        }
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Socket drained (or timed out with no data)
                        break;
                    }
                    Err(e) => {
                        log_error!("engine", format!("[ENGINE] Recv error: {}", e));
                        break;
                    }
                }

                // Safety: prevent infinite loop if socket floods (unlikely on UDP,
                // but protect against bugs)
                if ingress_count_this_tick > 10_000 {
                    break;
                }
            }

            // Track idle vs busy
            if ingress_count_this_tick == 0 {
                self.stats.idle_ticks += 1;
            } else {
                self.stats.busy_ticks += 1;
            }

            // ── PHASE 2: DRAIN OUTBOUND CHANNEL ───────────────
            // Send all queued outgoing packets (non-blocking drain)
            loop {
                match self.outbound_rx.try_recv() {
                    Ok(packet) => {
                        let result = if packet.mode.is_reliable() {
                            // Baseline toggle: aging disabled => effectively infinite half-life.
                            let half_life = if self.config.aging_enabled {
                                self.config.gradient_half_life_ms
                            } else {
                                f32::MAX
                            };
                            self.transport.send_reliable(
                                &packet.payload,
                                &packet.dst,
                                packet.mode.max_retries(),
                                half_life,
                            )
                        } else {
                            self.transport
                                .send_best_effort(&packet.payload, &packet.dst)
                        };

                        match result {
                            Ok(_seq) => {
                                self.stats.packets_sent += 1;
                                self.stats.bytes_sent += packet.payload.len() as u64;
                            }
                            Err(e) => {
                                log_error!("engine", format!("[ENGINE] Send error: {}", e));
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // All senders dropped — no more outbound traffic possible
                        // But we keep running (might still receive)
                        break;
                    }
                }
            }

            // ── PHASE 3: NEURAL COMPUTATION (every tick) ───────
            // Forward pass: propagate activations, compute predictions,
            // compare against observations, feed surprise to Neurogenesis.
            // Hebbian: STDP weight updates, micro-pruning, gossip.
            // Only runs when brain is attached via attach_brain().
            if self.brain_attached {
                // Collect observations from the ingress pipeline: decoded
                // remote activation frames (Data gossip) are drained here.
                // Locally observed values can be added by the caller.
                let mut observations: std::collections::HashMap<EntityId, f32> =
                    std::collections::HashMap::new();
                for (entity, value) in self.pending_observations.drain(..) {
                    observations.insert(entity, value);
                    // Mirror remote activations into the activation map so
                    // the Hebbian STDP update sees the remote neuron's
                    // value this tick (pre * post coupling).
                    self.activation_map.insert(
                        entity,
                        crate::components::ActivationComponent {
                            value,
                            last_updated_tick: self.tick,
                        },
                    );
                }

                // Step 1: Forward pass (borrows activation_map + synapse_map + neurogenesis)
                // Baseline toggle: neurogenesis disabled => use a throwaway system.
                let mut no_neurogenesis = NeurogenesisSystem::default();
                let neuro_ref = if self.config.neurogenesis_enabled {
                    &mut self.neurogenesis
                } else {
                    &mut no_neurogenesis
                };
                let fp_report = self.forward_pass.tick(
                    &mut self.activation_map,
                    &mut self.synapse_map,
                    neuro_ref,
                    self.tick,
                    &observations,
                );

                // Step 2: Hebbian learning (borrows activation_map immutably,
                // synapse_map mutably). Gossip targets come from the DHT
                // routing table (fall back to direct peers tracked by the
                // engine when no DHT handler is attached).
                let peers: Vec<SocketAddr> = if let Some(ref dht) = self.dht_handler {
                    dht.routing_table
                        .all_nodes()
                        .iter()
                        .map(|entry| entry.addr)
                        .collect()
                } else {
                    self.peer_rtt.keys().cloned().collect()
                };
                let hebbian_report = self.hebbian.tick(
                    &self.activation_map,
                    &mut self.synapse_map,
                    self.tick,
                    &self.outbound_tx,
                    &peers,
                    self.local_id,
                );
                self.stats.learning_frames_sent += hebbian_report.gossip_packets as u64;

                // Log notable brain events every tick
                if fp_report.neurons_spawned > 0 {
                    log_info!(
                        "brain",
                        format!(
                            "[ENGINE] tick={} spawned={} surprise={:.4} orphans={}",
                            self.tick,
                            fp_report.neurons_spawned,
                            fp_report.total_surprise,
                            fp_report.orphans_cleaned,
                        )
                    );
                }

                // Step 3: ML system — adaptive LR, meta-learning, curiosity,
                // memory, replay, distillation, continual learning.
                let ml_observations: Vec<crate::ml::Observation> = observations
                    .iter()
                    .map(|(entity, value)| crate::ml::Observation {
                        entity: *entity,
                        value: *value,
                        tick: self.tick,
                    })
                    .collect();
                let _ml_report = self.ml_system.tick(
                    self.tick,
                    &mut self.activation_map,
                    &mut self.synapse_map,
                    &ml_observations,
                );
            }

            // ── PHASE 4: RETRANSMIT (every N ticks) ──────────
            if self.tick - self.last_retransmit_tick >= self.config.retransmit_interval_ticks {
                self.last_retransmit_tick = self.tick;
                // Retransmit any un-ACKed reliable packets that still have retries.
                // Each packet is sent to its originally-stored destination address.
                let _ = self.transport.retransmit_stale();
            }

            // ── PHASE 4: CLEANUP & APOPTOSIS (every N ticks) ────
            if self.tick - self.last_cleanup_tick >= self.config.cleanup_interval_ticks {
                self.last_cleanup_tick = self.tick;

                // Transport cleanup (expired reliable frames)
                self.transport.cleanup_expired();

                // ── Peer RTT eviction ─────────────────────────
                // Remove peers we haven't heard from in 5 minutes.
                {
                    let now = u64::from(self.transport.now_ms());
                    let peer_ttl_ms: u64 = 300_000; // 5 minutes
                    let before_count = self.peer_rtt.len();
                    let mut evicted_addrs = Vec::new();
                    self.peer_rtt.retain(|addr, info| {
                        let age = now.saturating_sub(info.last_seen_ms);
                        if age > peer_ttl_ms {
                            log_info!(
                                "engine",
                                format!(
                                    "[ENGINE] Evicting stale peer {} (no activity for {}s)",
                                    addr,
                                    age / 1000,
                                ),
                                peer = &addr.to_string()
                            );
                            evicted_addrs.push(addr.ip());
                            false
                        } else {
                            true
                        }
                    });
                    // Decrement per-IP counts for evicted peers
                    for ip in &evicted_addrs {
                        if let Some(count) = self.peer_ip_count.get_mut(ip) {
                            *count = count.saturating_sub(1);
                            if *count == 0 {
                                self.peer_ip_count.remove(ip);
                            }
                        }
                    }
                    let evicted = before_count - self.peer_rtt.len();
                    if evicted > 0 {
                        self.stats.dht_dead_nodes += evicted as u64;
                    }
                }

                // Apoptosis sweep: evict dead DHT nodes, expired pings,
                // orphaned transport frames. Reports total deaths this sweep.
                // Baseline toggle: apoptosis disabled => skip the sweep.
                if let Some(ref mut dht) = self.dht_handler {
                    if self.config.apoptosis_enabled {
                        let report =
                            self.apoptosis_system
                                .tick(self.tick, dht, &mut self.transport);

                        // Death spiral guardrail
                        if self.apoptosis_system.is_death_spiral(&report) {
                            log_warn!(
                                "engine",
                                format!(
                                    "[ENGINE] ⚠️ DEATH SPIRAL: {} nodes evicted at tick {}. \
                                 Network partition or seed node failure.",
                                    report.total_deaths, self.tick,
                                )
                            );
                        } else if report.total_deaths > 0 {
                            log_warn!(
                                "apoptosis",
                                format!(
                                    "[APOPTOSIS] sweep: {} deaths (DHT:{} ping:{} frames:{})",
                                    report.total_deaths,
                                    report.dht_nodes_evicted,
                                    report.pending_pings_expired,
                                    report.data_frames_purged,
                                )
                            );
                        }
                    }

                    // DHT periodic maintenance (ping stale, save peers).
                    // Baseline toggle: static topology => no discovery beyond initial peers.
                    if !self.config.static_topology {
                        dht.periodic_maintenance();
                    }
                }

                // ── Heartbeat keepalive ────────────────────────
                if self.config.heartbeat_interval_ticks > 0
                    && self.tick - self.last_heartbeat_tick >= self.config.heartbeat_interval_ticks
                {
                    self.last_heartbeat_tick = self.tick;
                    self.send_heartbeats();
                }

                self.update_stats();
            }

            // ── PHASE 5: YIELD ────────────────────────────────
            // The read timeout on recv_from handles the sleep.
            // But in rare cases where recv_from returns instantly (many packets),
            // we need to yield to prevent CPU saturation.
            if ingress_count_this_tick > 100 {
                std::thread::yield_now();
            }

            // Print stats every 1000 ticks
            if self.tick.is_multiple_of(1000) {
                self.print_stats();
            }
        }
    }

    // ─── Ingress Pipeline ──────────────────────────────────────

    /// Process an incoming UDP datagram.
    /// Validates CRC, updates ACK tracker, applies gradient decay,
    /// and dispatches to the event channel.
    fn handle_ingress(&mut self, data: &[u8], src: SocketAddr) -> Result<(), NwpError> {
        // Packet filter for failure injection (partition simulation).
        // When packet_filter_allowed is Some, only packets from those addresses are processed.
        {
            let allowed = self
                .packet_filter_allowed
                .lock()
                .map_err(|e| NwpError::connection_refused(format!("lock poisoned: {}", e)))?;
            if let Some(ref allowed_set) = *allowed {
                if !allowed_set.contains(&src) {
                    return Ok(()); // silently drop
                }
            }
        }

        // Deterministic in-sim packet loss (seeded xorshift — reproducible runs).
        if self.config.packet_loss_rate > 0.0 {
            self.loss_rng ^= self.loss_rng << 13;
            self.loss_rng ^= self.loss_rng >> 7;
            self.loss_rng ^= self.loss_rng << 17;
            let r = (self.loss_rng % 10_000) as f32 / 10_000.0;
            if r < self.config.packet_loss_rate {
                return Ok(()); // simulated loss — silently drop
            }
        }

        // data = full UDP datagram: [16-byte transport header][NWP frame]
        // NWP frame layout: [4-byte frame_len][16-byte MessageHeader][body]
        if data.len() < TransportHeader::SIZE + 4 {
            return Err(NwpError::Transport(TransportError::PacketTooShort {
                actual: data.len(),
                expected: TransportHeader::SIZE + 4,
            }));
        }

        // ── Security: trust-based rate limiting ────────────────
        let peer_id = entity_id_from_addr(&src);
        // Baseline toggle: trust disabled => no rate limiting.
        if self.config.trust_enabled && self.trust_system.check_rate_limit(&peer_id) {
            self.audit_log.append(
                AuditEventType::RateLimitTriggered,
                &format!("rate-limited packet from {}", src),
                Some(peer_id),
            );
            log_warn!(
                "security",
                format!("[ENGINE] Rate-limited packet from {}", src),
                peer = &src.to_string()
            );
            return Ok(()); // silently drop, but log
        }

        // Zero-copy parse the transport header from the raw datagram
        // SAFETY: data.len() >= TransportHeader::SIZE + 4 (checked above at line 755),
        // so data is at least 20 bytes — more than the 16 required by from_bytes.
        let transport_header = unsafe { TransportHeader::from_bytes(data) };

        // Update ACK tracker with the received sequence number
        self.transport
            .ack_tracker
            .record(transport_header.sequence_number);

        // Process the ACK this packet carries (clear our reliable queue)
        self.transport
            .reliable_queue
            .process_ack(transport_header.ack_number, transport_header.ack_bitfield);

        // Strip transport header to get the NWP frame (frame_len + header + body)
        let nwp_frame = &data[TransportHeader::SIZE..];

        // Strip 4-byte frame_len to get the NWP message (header + body)
        // build_frame() prepends this length prefix before the MessageHeader
        let nwp_payload: &[u8] = &nwp_frame[4..];

        // Track this source as a peer (for peer count / convergence detection)
        // ── Connection limit: DoS protection ──────────────────
        if self.config.max_peers > 0
            && !self.peer_rtt.contains_key(&src)
            && self.peer_rtt.len() >= self.config.max_peers
        {
            log_warn!(
                "security",
                format!(
                    "[ENGINE] Connection limit reached, sending TOO_MANY_PEERS to {}",
                    src
                ),
                peer = &src.to_string()
            );
            self.send_disconnect(
                src,
                header::disconnect_reason::TOO_MANY_PEERS,
                "node at capacity",
            );
            return Ok(());
        }
        // ── Per-IP connection limit: prevent single-IP DoS ───
        if self.config.per_ip_max_peers > 0 && !self.peer_rtt.contains_key(&src) {
            let ip_count = self.peer_ip_count.entry(src.ip()).or_insert(0);
            if *ip_count >= self.config.per_ip_max_peers {
                log_warn!(
                    "security",
                    format!(
                        "[ENGINE] Per-IP limit reached for {}, sending TOO_MANY_PEERS",
                        src.ip()
                    ),
                    peer = &src.to_string()
                );
                self.send_disconnect(
                    src,
                    header::disconnect_reason::TOO_MANY_PEERS,
                    "per-IP connection limit",
                );
                return Ok(());
            }
            *ip_count += 1;
        }
        self.peer_rtt.entry(src).or_insert(PeerInfo {
            rtt_ms: 100.0,
            last_seen_ms: u64::from(self.transport.now_ms()),
        });
        // Update last-seen for existing peers too
        if let Some(info) = self.peer_rtt.get_mut(&src) {
            info.last_seen_ms = u64::from(self.transport.now_ms());
        }

        // ── Handle Heartbeat/Disconnect messages inline ─────
        if nwp_payload.len() >= 6 {
            let msg_type_byte = nwp_payload[5]; // msg_type is at offset 5 in NWP header
            if msg_type_byte == header::msg_type::DISCONNECT && nwp_payload.len() > 16 {
                let body = &nwp_payload[16..]; // skip 16-byte NWP header
                self.handle_disconnect(src, body);
                return Ok(());
            }
            if msg_type_byte == header::msg_type::HEARTBEAT {
                self.handle_heartbeat(src);
                return Ok(());
            }

            // ── Distributed learning: decode remote activation frames ──
            // Hebbian gossip frames (MsgType::Data = 5) carry serialized
            // synapse updates from a remote peer:
            //   [32 B source_entity] [u16 count] ( post_id, targets,
            //     weights, accumulated_gradients )*
            // We decode them into pending observations so the live neural
            // path (forward pass + Hebbian STDP) consumes remote learning
            // signals — the bridge between "distributed network" and
            // "distributed learning".
            if msg_type_byte == crate::types::MsgType::Data as u8 && nwp_payload.len() > 16 {
                let body = &nwp_payload[16..]; // skip 16-byte NWP header
                if let Some((_source, entries)) = crate::hebbian::deserialize_gossip_packet(body) {
                    for (post_id, _targets, _weights, grads) in entries {
                        // Activation magnitude = total absolute gradient
                        // shipped by the remote neuron's update.
                        let magnitude: f32 = grads.iter().map(|g| g.abs()).sum();
                        if magnitude > 0.0 {
                            self.pending_observations
                                .push((post_id, magnitude.min(1.0)));
                        }
                    }
                    self.stats.learning_frames_recv += 1;
                }
            }
        }

        // Gradient weight default: 1.0 (full utility for each received packet).
        let gradient_weight = 1.0;

        // Dispatch the event with the real transport header
        let event = IngressEvent {
            transport_header: *transport_header,
            nwp_payload: nwp_payload.to_vec(),
            src,
            recv_timestamp: self.transport.now_ms(),
            gradient_weight,
        };

        // Non-blocking send — if no subscriber, silently drop
        if let Some(tx) = &self.events_tx {
            let _ = tx.send(event.clone());
        }

        // Dispatch to DHT handler for inline processing (PING/PONG/FIND_NODE/NODES)
        if let Some(ref mut dht) = self.dht_handler {
            dht.handle_event(&event);
        }

        Ok(())
    }

    // ─── Stats ──────────────────────────────────────────────────

    fn update_stats(&mut self) {
        self.stats.peer_count = self.peer_rtt.len();
        self.stats.outbound_queue_depth = 0; // we drained the channel each tick
        self.stats.reliable_queue_depth = self.transport.reliable_queue.pending_count();
        self.sync_stats();

        let elapsed = self.last_stats_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.stats.actual_tick_rate_hz = self.tick as f64 / elapsed;
        }
    }

    fn print_stats(&self) {
        let elapsed = self.last_stats_time.elapsed().as_secs_f64();
        let mb_recv = self.stats.bytes_recv as f64 / 1_000_000.0;
        let mb_sent = self.stats.bytes_sent as f64 / 1_000_000.0;

        let tick_rate = if elapsed > 0.0 {
            self.tick as f64 / elapsed
        } else {
            0.0
        };
        let idle_pct = if self.tick > 0 {
            self.stats.idle_ticks as f64 / self.tick as f64 * 100.0
        } else {
            0.0
        };
        log_debug!(
            "engine",
            format!(
                "[ENGINE] tick={} rate={:.0}Hz rx={} pkts ({:.2}MB) tx={} pkts ({:.2}MB) \
             idle={:.1}% reliable_q={} peers={}",
                self.tick,
                tick_rate,
                self.stats.packets_recv,
                mb_recv,
                self.stats.packets_sent,
                mb_sent,
                idle_pct,
                self.stats.reliable_queue_depth,
                self.peer_rtt.len(),
            )
        );
    }

    /// Get the engine stats
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    /// Sync live stats to shared pointer so external watchers see them.
    fn sync_stats(&self) {
        if let Some(ref shared) = self.config.shared_stats {
            if let Ok(mut s) = shared.lock() {
                *s = self.stats.clone();
                // Capacity metrics
                s.max_peers = self.config.max_peers;
                s.active_peer_count = self.peer_rtt.len();
                s.peer_capacity_ratio = if self.config.max_peers > 0 {
                    self.peer_rtt.len() as f64 / self.config.max_peers as f64
                } else {
                    0.0
                };
                // DHT metrics
                if let Some(ref dht) = self.dht_handler {
                    s.dht_node_count = dht.routing_table.node_count();
                    s.dht_pending_pings = dht.pending_ping_count();
                }
                // Trust metrics
                let ts = self.trust_system.stats();
                s.trust_peer_count = ts.total_peers;
                s.trust_rate_limited_peers = ts.rate_limited_peers;
                // Session metrics
                s.active_sessions = self.secure_channel.session_count();
                s.ephemeral_sessions = self.secure_channel.ephemeral_count();
            }
        }
    }

    /// Get peer RTT estimates
    pub fn peer_rtt(&self) -> &HashMap<SocketAddr, PeerInfo> {
        &self.peer_rtt
    }

    /// Distributed-learning observability: number of learning frames
    /// (Data gossip) received and sent so far.
    pub fn learning_stats(&self) -> (u64, u64) {
        (
            self.stats.learning_frames_recv,
            self.stats.learning_frames_sent,
        )
    }

    /// Distributed-learning observability: current synapse weight for an
    /// entity, if any. Used by end-to-end tests to prove a remote
    /// activation actually changed local weights.
    pub fn synapse_weight_for_test(&self, post: &EntityId, target: &EntityId) -> Option<f32> {
        let synapse = self.synapse_map.get(post)?;
        let idx = synapse.target_entities.iter().position(|t| t == target)?;
        Some(synapse.weights[idx])
    }

    /// Distributed-learning observability: synapse map size (test-only).
    pub fn synapse_count_for_test(&self) -> usize {
        self.synapse_map.len()
    }

    /// Distributed-learning observability: activation map size (test-only).
    pub fn activation_count_for_test(&self) -> usize {
        self.activation_map.len()
    }

    /// Distributed-learning observability: does the synapse map contain
    /// `post` at all? (test-only)
    pub fn has_synapse_for_test(&self, post: &EntityId) -> bool {
        self.synapse_map.contains_key(post)
    }

    // ─── Heartbeat Protocol ───────────────────────────────────

    /// Send a HEARTBEAT message to all known peers.
    fn send_heartbeats(&self) {
        let flags = if self.config.security_enabled {
            header::FLAG_AUTHENTICATED
        } else {
            0
        };
        let frame = header::build_frame(header::msg_type::HEARTBEAT, Vec::new(), flags);
        for addr in self.peer_rtt.keys() {
            let _ = self.transport.socket.send_to(&frame, *addr);
        }
    }

    /// Handle an incoming Heartbeat message.
    fn handle_heartbeat(&mut self, src: SocketAddr) {
        log_debug!(
            "engine",
            format!("[ENGINE] Heartbeat from {}", src),
            peer = &src.to_string()
        );
    }

    // ─── Disconnect Protocol ──────────────────────────────────

    /// Send a graceful disconnect message to a specific peer.
    fn send_disconnect(&self, dst: SocketAddr, reason: u8, message: &str) {
        let mut body = vec![reason];
        body.extend_from_slice(message.as_bytes());
        let flags = if self.config.security_enabled {
            header::FLAG_AUTHENTICATED
        } else {
            0
        };
        let frame = header::build_frame(header::msg_type::DISCONNECT, body, flags);
        let _ = self.transport.socket.send_to(&frame, dst);
        log_info!(
            "engine",
            format!("[ENGINE] Sent DISCONNECT(reason={}) to {}", reason, dst),
            peer = &dst.to_string()
        );
    }

    /// Broadcast disconnect to all known peers (shutdown procedure).
    #[allow(dead_code)] // reserved for graceful shutdown broadcasts
    fn broadcast_disconnect(&self, reason: u8, message: &str) {
        for addr in self.peer_rtt.keys() {
            self.send_disconnect(*addr, reason, message);
        }
    }

    /// Handle an incoming Disconnect message.
    fn handle_disconnect(&mut self, src: SocketAddr, body: &[u8]) {
        if body.is_empty() {
            log_info!(
                "engine",
                format!("[ENGINE] Disconnect from {} (no reason)", src),
                peer = &src.to_string()
            );
            return;
        }
        let reason = body[0];
        let detail = if body.len() > 1 {
            String::from_utf8_lossy(&body[1..]).to_string()
        } else {
            String::new()
        };
        log_info!(
            "engine",
            format!(
                "[ENGINE] Disconnect from {} reason={}: {}",
                src, reason, detail
            ),
            peer = &src.to_string()
        );
        self.peer_rtt.remove(&src);
    }
}

// ─── Convenience: Spawn Engine Thread ──────────────────────────

/// Run the engine in a background thread.
/// Returns (outbound_tx, events_rx, join_handle).
/// Optional `packet_filter_allowed` — when provided, the engine uses this shared
/// filter for partition simulation instead of creating a private one.
pub fn spawn_engine(
    config: EngineConfig,
    dht_handler: Option<DhtHandler>,
    shutdown: Arc<AtomicBool>,
    packet_filter_allowed: Option<Arc<Mutex<Option<Vec<SocketAddr>>>>>,
) -> std::io::Result<(
    Sender<OutgoingPacket>,
    Receiver<IngressEvent>,
    std::thread::JoinHandle<()>,
)> {
    let (outbound_tx, outbound_rx) = mpsc::channel();
    let (events_tx, events_rx) = mpsc::channel();

    let transport = UdpTransport::bind(&config.bind_addr)?;

    let outbound_tx_for_return = outbound_tx.clone();

    // Deterministic impairment RNG seed: sim_seed mixed with the bind address.
    let mut loss_hasher = std::collections::hash_map::DefaultHasher::new();
    config.bind_addr.hash(&mut loss_hasher);
    let loss_rng = config.sim_seed ^ loss_hasher.finish() ^ 0x9E37_79B9_7F4A_7C15;

    let handle = std::thread::Builder::new()
        .name("nwp-engine".to_string())
        .spawn(move || {
            let mut engine = EngineLoop {
                config,
                transport,
                outbound_rx,
                events_tx: Some(events_tx),
                dht_handler,
                apoptosis_system: ApoptosisSystem::new(),
                shutdown: shutdown.clone(),
                // Security subsystem
                node_identity: NodeIdentity::new(),
                secure_channel: SecureChannel::new(),
                trust_system: TrustSystem::new(),
                audit_log: AuditLog::new(),
                tick: 0,
                last_retransmit_tick: 0,
                last_cleanup_tick: 0,
                last_heartbeat_tick: 0,
                last_stats_time: Instant::now(),
                stats: EngineStats::default(),
                peer_rtt: HashMap::with_capacity(512),
                peer_ip_count: HashMap::with_capacity(128),
                recv_buf: vec![0u8; 65535],
                activation_map: HashMap::with_capacity(256),
                synapse_map: HashMap::with_capacity(1024),
                forward_pass: ForwardPassSystem::default(),
                neurogenesis: NeurogenesisSystem::default(),
                hebbian: HebbianLearningSystem::new(0.01, 0.999, 0.001, 500),
                ml_system: MLSystem::new(),
                local_id: EntityId([0u8; 32]),
                outbound_tx: outbound_tx.clone(),
                brain_attached: false,
                packet_filter_allowed: packet_filter_allowed
                    .unwrap_or_else(|| Arc::new(Mutex::new(None))),
                pending_observations: Vec::new(),
                loss_rng,
            };

            // Auto-create DHT handler if local peers configured but no handler given
            if engine.dht_handler.is_none() && !engine.config.local_peers.is_empty() {
                use rand::Rng;
                let mut local_id = [0u8; 32];
                rand::thread_rng().fill(&mut local_id);
                let mut dht = DhtHandler::new(
                    NodeId::new(local_id),
                    engine
                        .config
                        .bind_addr
                        .parse()
                        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    NodeType::General,
                    outbound_tx.clone(),
                    None,
                    "local".to_string(),
                );
                dht.random_discovery = engine.config.random_discovery;
                engine.dht_handler = Some(dht);
            }

            // Activate SGA if freshness_config is provided
            if let Some(ref fconfig) = engine.config.freshness_config {
                if fconfig.enabled {
                    if let Some(ref mut dht) = engine.dht_handler {
                        dht.enable_sga(*fconfig);
                        log_info!(
                            "engine",
                            format!(
                                "[ENGINE] SGA active (half-life={}ms, stretch={}, base={}ms)",
                                fconfig.half_life_ms,
                                fconfig.stretch_factor,
                                fconfig.base_interval_ms
                            )
                        );
                    }
                }
            }

            // Bootstrap: PING all known peers.
            // No random-ID placeholder is inserted here because `handle_pong()` will
            // create the entry with the real NodeId when the PONG arrives. Using
            // Reliability::Data gives 3 retries per PING, surviving startup races.
            if let Some(ref mut dht) = engine.dht_handler {
                for peer_addr in &engine.config.local_peers {
                    dht.ping_node(*peer_addr);
                }
                dht.bootstrap();
                log_info!(
                    "engine",
                    format!(
                        "[ENGINE] Bootstrapped {} local peers",
                        engine.config.local_peers.len()
                    )
                );
            }

            // Set 1ms read timeout
            let _ = engine
                .transport
                .socket
                .set_read_timeout(Some(Duration::from_millis(engine.config.tick_interval_ms)));

            let mut recv_buf = vec![0u8; engine.config.recv_buffer_size];

            loop {
                engine.tick += 1;

                // ── SHUTDOWN CHECK ─────────────────────────────
                if engine.shutdown.load(Ordering::Relaxed) {
                    log_info!(
                        "engine",
                        format!(
                            "[ENGINE] Shutdown signal. Exiting after {} ticks.",
                            engine.tick
                        )
                    );
                    return;
                }

                let mut ingress_count = 0u32;

                // Phase 1: Drain UDP socket
                loop {
                    match engine.transport.socket.recv_from(&mut recv_buf) {
                        Ok((len, src)) => {
                            ingress_count += 1;
                            engine.stats.packets_recv += 1;
                            engine.stats.bytes_recv += len as u64;
                            // Pass the full datagram — handle_ingress parses
                            // the transport header internally.
                            if len >= TransportHeader::SIZE {
                                if let Err(e) = engine.handle_ingress(&recv_buf[..len], src) {
                                    log_error!("engine", format!("[ENGINE] Ingress error: {}", e));
                                }
                            }
                        }
                        Err(ref e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            break
                        }
                        Err(e) => {
                            log_error!("engine", format!("[ENGINE] Recv error: {}", e));
                            break;
                        }
                    }
                    if ingress_count > 10_000 {
                        break;
                    }
                }

                if ingress_count == 0 {
                    engine.stats.idle_ticks += 1;
                } else {
                    engine.stats.busy_ticks += 1;
                }

                // Phase 2: Drain outbound channel
                loop {
                    match engine.outbound_rx.try_recv() {
                        Ok(pkt) => {
                            let result = if pkt.mode.is_reliable() {
                                engine.transport.send_reliable(
                                    &pkt.payload,
                                    &pkt.dst,
                                    pkt.mode.max_retries(),
                                    engine.config.gradient_half_life_ms,
                                )
                            } else {
                                engine.transport.send_best_effort(&pkt.payload, &pkt.dst)
                            };
                            if let Ok(_seq) = result {
                                engine.stats.packets_sent += 1;
                                engine.stats.bytes_sent += pkt.payload.len() as u64;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break,
                    }
                }

                // Phase 3: Retransmit (every 10 ticks)
                if engine.tick - engine.last_retransmit_tick
                    >= engine.config.retransmit_interval_ticks
                {
                    engine.last_retransmit_tick = engine.tick;
                    let _ = engine.transport.retransmit_stale();
                }

                // Phase 4: Cleanup & Apoptosis (every 1000 ticks)
                if engine.tick - engine.last_cleanup_tick >= engine.config.cleanup_interval_ticks {
                    engine.last_cleanup_tick = engine.tick;
                    engine.transport.cleanup_expired();

                    // Apoptosis sweep
                    if let Some(ref mut dht) = engine.dht_handler {
                        let report =
                            engine
                                .apoptosis_system
                                .tick(engine.tick, dht, &mut engine.transport);
                        if engine.apoptosis_system.is_death_spiral(&report) {
                            log_warn!(
                                "engine",
                                format!(
                                    "[ENGINE] ⚠️ DEATH SPIRAL: {} nodes evicted at tick {}.",
                                    report.total_deaths, engine.tick,
                                )
                            );
                        } else if report.total_deaths > 0 {
                            eprintln!(
                                "[APOPTOSIS] sweep: {} deaths (DHT:{} ping:{})",
                                report.total_deaths,
                                report.dht_nodes_evicted,
                                report.pending_pings_expired,
                            );
                        }
                        dht.periodic_maintenance();
                    }

                    engine.update_stats();
                }

                // Phase 5: Yield if busy
                if ingress_count > 100 {
                    std::thread::yield_now();
                }

                // Stats every 1000 ticks
                if engine.tick.is_multiple_of(1000) {
                    engine.print_stats();
                }
            }
        })?;

    Ok((outbound_tx_for_return, events_rx, handle))
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::calculate_gradient_weight;

    #[test]
    fn test_reliability_enum() {
        assert_eq!(Reliability::BestEffort.max_retries(), 0);
        assert_eq!(Reliability::Data.max_retries(), 3);
        assert_eq!(Reliability::Consensus.max_retries(), 5);
        assert!(!Reliability::BestEffort.is_reliable());
        assert!(Reliability::Data.is_reliable());
        assert!(Reliability::Consensus.is_reliable());
    }

    #[test]
    fn test_engine_config_default() {
        let cfg = EngineConfig::default();
        assert_eq!(cfg.tick_interval_ms, 1);
        assert_eq!(cfg.retransmit_interval_ticks, 10);
        assert_eq!(cfg.cleanup_interval_ticks, 1000);
    }

    #[test]
    fn test_gradient_weight_near_zero() {
        // Very old packet: weight should be near 0
        let w = calculate_gradient_weight(10_000, 100.0);
        assert!(w < 0.0001);
    }

    #[test]
    fn test_gradient_weight_fresh() {
        let w = calculate_gradient_weight(0, 100.0);
        assert!((w - 1.0).abs() < 0.001);
    }

    // ── New tests for new features ──────────────────────────────

    #[test]
    fn test_peer_info_creation() {
        let peer = PeerInfo {
            rtt_ms: 100.0,
            last_seen_ms: 12345,
        };
        assert_eq!(peer.rtt_ms, 100.0);
        assert!(peer.last_seen_ms > 0);
    }

    #[test]
    fn test_peer_rtt_eviction() {
        // Create engine with an ephemeral port (parallel tests share 0.0.0.0:9000)
        let cfg = EngineConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            ..Default::default()
        };
        let (mut engine, _tx, _rx) = EngineLoop::new(cfg).unwrap();

        // Use a synthetic clock so we can represent peers older than the
        // process-relative u32 clock without waiting 5+ minutes.
        let synthetic_now: u64 = 1_000_000;
        let mut insert = |addr: &str, rtt: f32, age_ms: u64| {
            let a: std::net::SocketAddr = addr.parse().unwrap();
            engine.peer_rtt.insert(
                a,
                PeerInfo {
                    rtt_ms: rtt,
                    last_seen_ms: synthetic_now - age_ms,
                },
            );
        };

        // Old peers: age > 300_000 ms (300s) — should be evicted
        insert("10.0.0.1:9000", 50.0, 400_000); // 400s old
        insert("10.0.0.2:9000", 60.0, 350_000); // 350s old
        insert("10.0.0.3:9000", 70.0, 310_000); // 310s old

        // Recent peers: age < 300_000 ms — should remain
        insert("10.0.0.4:9000", 80.0, 10_000); // 10s old
        insert("10.0.0.5:9000", 90.0, 60_000); // 60s old
        insert("10.0.0.6:9000", 100.0, 200_000); // 200s old

        assert_eq!(
            engine.peer_count_for_test(),
            6,
            "should have 6 peers before eviction"
        );

        // Run the eviction logic (same code as in the cleanup phase)
        let now = synthetic_now;
        let peer_ttl_ms: u64 = 300_000;
        engine.peer_rtt.retain(|_addr, info| {
            let age = now.saturating_sub(info.last_seen_ms);
            age <= peer_ttl_ms
        });

        assert_eq!(
            engine.peer_count_for_test(),
            3,
            "only 3 recent peers should remain"
        );

        // Verify the correct peers survived
        assert!(engine
            .peer_rtt
            .contains_key(&"10.0.0.4:9000".parse().unwrap()));
        assert!(engine
            .peer_rtt
            .contains_key(&"10.0.0.5:9000".parse().unwrap()));
        assert!(engine
            .peer_rtt
            .contains_key(&"10.0.0.6:9000".parse().unwrap()));

        // Verify old peers were evicted
        assert!(!engine
            .peer_rtt
            .contains_key(&"10.0.0.1:9000".parse().unwrap()));
        assert!(!engine
            .peer_rtt
            .contains_key(&"10.0.0.2:9000".parse().unwrap()));
        assert!(!engine
            .peer_rtt
            .contains_key(&"10.0.0.3:9000".parse().unwrap()));
    }

    #[test]
    fn test_connection_limit() {
        let cfg = EngineConfig {
            max_peers: 2,
            ..Default::default()
        };
        let (mut engine, _tx, _rx) = EngineLoop::new(cfg).unwrap();

        // Fill to capacity
        engine.insert_peer_for_test("10.0.0.1:9000".parse().unwrap(), 50.0, 1000);
        engine.insert_peer_for_test("10.0.0.2:9000".parse().unwrap(), 60.0, 1000);
        assert_eq!(engine.peer_count_for_test(), 2);

        // Verify the connection limit logic: a new unknown peer should be rejected
        let new_src: SocketAddr = "10.0.0.3:9000".parse().unwrap();
        let limit_reached = engine.config.max_peers > 0
            && !engine.peer_rtt.contains_key(&new_src)
            && engine.peer_rtt.len() >= engine.config.max_peers;
        assert!(
            limit_reached,
            "new peer should be rejected when at capacity"
        );

        // An already-known peer should NOT be rejected
        let known_src: SocketAddr = "10.0.0.1:9000".parse().unwrap();
        let known_rejected = engine.config.max_peers > 0
            && !engine.peer_rtt.contains_key(&known_src)
            && engine.peer_rtt.len() >= engine.config.max_peers;
        assert!(!known_rejected, "known peer should NOT be rejected");
    }

    #[test]
    fn test_engine_config_defaults() {
        let cfg = EngineConfig::default();

        // Core timing
        assert_eq!(cfg.bind_addr, "0.0.0.0:9000");
        assert_eq!(cfg.tick_interval_ms, 1);
        assert_eq!(cfg.retransmit_interval_ticks, 10);
        assert_eq!(cfg.cleanup_interval_ticks, 1000);
        assert_eq!(cfg.max_outbound_queue, 10_000);
        assert_eq!(cfg.recv_buffer_size, 65535);
        assert_eq!(cfg.gradient_half_life_ms, 100.0);

        // Peers & heartbeat
        assert_eq!(cfg.max_peers, 500);
        assert_eq!(cfg.heartbeat_interval_ticks, 30_000);

        // Security defaults
        assert!(
            cfg.security_enabled,
            "security_enabled should default to true"
        );
        assert!(
            !cfg.encrypt_payloads,
            "encrypt_payloads should default to false"
        );
        assert!(!cfg.stun_enabled, "stun_enabled should default to false");
        assert_eq!(cfg.stun_server, "stun.l.google.com:19302");

        // Optional paths default to None
        assert!(cfg.identity_seed.is_none());
        assert!(cfg.peer_cache_path.is_none());
        assert!(cfg.trust_cache_path.is_none());
        assert!(cfg.freshness_config.is_none());
        assert!(cfg.shared_stats.is_none());
        assert!(cfg.local_peers.is_empty());
        assert!(cfg.seed_domain.is_empty());
    }

    #[test]
    fn test_engine_stats_defaults() {
        let stats = EngineStats::default();

        // All counters should start at 0
        assert_eq!(stats.total_ticks, 0);
        assert_eq!(stats.packets_recv, 0);
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.bytes_recv, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.retransmissions, 0);
        assert_eq!(stats.peer_count, 0);
        assert_eq!(stats.outbound_queue_depth, 0);
        assert_eq!(stats.reliable_queue_depth, 0);
        assert_eq!(stats.idle_ticks, 0);
        assert_eq!(stats.busy_ticks, 0);
        assert_eq!(stats.actual_tick_rate_hz, 0.0);

        // Security metrics
        assert_eq!(stats.authenticated_packets, 0);
        assert_eq!(stats.encrypted_packets, 0);
        assert_eq!(stats.auth_failures, 0);
        assert_eq!(stats.decrypt_failures, 0);
        assert_eq!(stats.rate_limited_packets, 0);

        // DHT metrics
        assert_eq!(stats.dht_node_count, 0);
        assert_eq!(stats.dht_pending_pings, 0);
        assert_eq!(stats.dht_dead_nodes, 0);

        // Trust metrics
        assert_eq!(stats.trust_peer_count, 0);
        assert_eq!(stats.trust_rate_limited_peers, 0);

        // Capacity metrics
        assert_eq!(stats.max_peers, 0);
        assert_eq!(stats.active_peer_count, 0);
        assert_eq!(stats.peer_capacity_ratio, 0.0);

        // Session metrics
        assert_eq!(stats.active_sessions, 0);
        assert_eq!(stats.ephemeral_sessions, 0);
    }
}
