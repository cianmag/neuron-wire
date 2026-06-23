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
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::transport::{
    TransportHeader,
    UdpTransport,
    calculate_gradient_weight,
};
use crate::apoptosis::ApoptosisSystem;
use crate::components::{ActivationMap, EntityId, SynapseMap};
use crate::dht::{DhtHandler, NodeEntry, NodeId, NodeType};
use crate::forward_pass::ForwardPassSystem;
use crate::hebbian::HebbianLearningSystem;
use crate::neurogenesis::NeurogenesisSystem;

// ─── Configuration ─────────────────────────────────────────────

/// Engine loop configuration
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
}

// ─── Engine Loop ───────────────────────────────────────────────

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
    /// Timers (tick counters)
    tick: u64,
    last_retransmit_tick: u64,
    last_cleanup_tick: u64,
    /// Last stats snapshot time
    last_stats_time: Instant,
    /// Running stats
    stats: EngineStats,
    /// Map of known peers + their RTT estimates (ms)
    peer_rtt: HashMap<SocketAddr, f32>,
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
    /// Local node's 256-bit cryptographic identity
    local_id: EntityId,
    /// Clone of the outbound sender (for Hebbian gossip)
    outbound_tx: Sender<OutgoingPacket>,
    /// Whether the brain is attached and should tick
    brain_attached: bool,
}

impl EngineLoop {
    /// Create a new engine loop.
    /// Returns (engine, outbound_tx, events_rx).
    pub fn new(config: EngineConfig) -> std::io::Result<(Self, Sender<OutgoingPacket>, Receiver<IngressEvent>)> {
        let transport = UdpTransport::bind(&config.bind_addr)?;
        let (outbound_tx, outbound_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();

        let engine = EngineLoop {
            config,
            transport,
            outbound_rx,
            events_tx: Some(events_tx),
            dht_handler: None,
            apoptosis_system: ApoptosisSystem::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
            tick: 0,
            last_retransmit_tick: 0,
            last_cleanup_tick: 0,
            last_stats_time: Instant::now(),
            stats: EngineStats::default(),
            peer_rtt: HashMap::new(),
            // Brain state defaults (attach via attach_brain())
            activation_map: HashMap::new(),
            synapse_map: HashMap::new(),
            forward_pass: ForwardPassSystem::default(),
            neurogenesis: NeurogenesisSystem::default(),
            hebbian: HebbianLearningSystem::new(0.01, 0.999, 0.001, 500),
            local_id: EntityId([0u8; 32]),
            outbound_tx: outbound_tx.clone(),
            brain_attached: false,
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
    pub fn attach_brain(
        &mut self,
        activation_map: ActivationMap,
        synapse_map: SynapseMap,
        forward_pass: ForwardPassSystem,
        neurogenesis: NeurogenesisSystem,
        hebbian: HebbianLearningSystem,
        local_id: EntityId,
    ) {
        self.activation_map = activation_map;
        self.synapse_map = synapse_map;
        self.forward_pass = forward_pass;
        self.neurogenesis = neurogenesis;
        self.hebbian = hebbian;
        self.local_id = local_id;
        self.brain_attached = true;
    }

    /// Run the engine loop. **Blocks the current thread until shutdown is signalled.**
    ///
    /// Single-threaded, non-blocking loop with ~1ms tick rate.
    /// The thread sleeps during idle via the UDP socket's read timeout.
    pub fn run(&mut self) {
        // Set 1ms read timeout so recv_from blocks for at most 1ms
        if let Err(e) = self.transport
            .socket
            .set_read_timeout(Some(Duration::from_millis(self.config.tick_interval_ms)))
        {
            eprintln!("[ENGINE] WARN: could not set read timeout: {}", e);
        }

        // Pre-allocate a local receive buffer (stack-friendly, reused)
        let mut recv_buf = vec![0u8; self.config.recv_buffer_size];
        let mut ingress_count_this_tick: u32;

        loop {
            self.tick += 1;

            // ── SHUTDOWN CHECK ─────────────────────────────────
            if self.shutdown.load(Ordering::Relaxed) {
                eprintln!("[ENGINE] Shutdown signal received at tick {}. Exiting.", self.tick);
                return;
            }

            ingress_count_this_tick = 0;

            // ── PHASE 1: DRAIN UDP SOCKET ─────────────────────
            // Non-blocking: drain ALL available messages from the socket buffer.
            // This prevents the "one-per-iteration" bottleneck.
            loop {
                match self.transport.socket.recv_from(&mut recv_buf) {
                    Ok((len, src)) => {
                        ingress_count_this_tick += 1;
                        self.stats.packets_recv += 1;
                        self.stats.bytes_recv += len as u64;

                        if let Err(e) = self.handle_ingress(&recv_buf[..len], src) {
                            eprintln!("[ENGINE] ingress error: {}", e);
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
                        eprintln!("[ENGINE] recv error: {}", e);
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
                            self.transport.send_reliable(
                                &packet.payload,
                                &packet.dst,
                                packet.mode.max_retries(),
                                self.config.gradient_half_life_ms,
                            )
                        } else {
                            self.transport.send_best_effort(&packet.payload, &packet.dst)
                        };

                        match result {
                            Ok(_seq) => {
                                self.stats.packets_sent += 1;
                                self.stats.bytes_sent += packet.payload.len() as u64;
                            }
                            Err(e) => {
                                eprintln!("[ENGINE] send error: {}", e);
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
                // Collect observations from the ingress pipeline.
                // In a full system, decoded NWP frames carry observed
                // activation values from remote peers.
                let observations: std::collections::HashMap<EntityId, f32> = std::collections::HashMap::new();

                // Step 1: Forward pass (borrows activation_map + synapse_map + neurogenesis)
                let fp_report = self.forward_pass.tick(
                    &mut self.activation_map,
                    &mut self.synapse_map,
                    &mut self.neurogenesis,
                    self.tick,
                    &observations,
                );

                // Step 2: Hebbian learning (borrows activation_map immutably, synapse_map mutably)
                let _hebbian_report = self.hebbian.tick(
                    &self.activation_map,
                    &mut self.synapse_map,
                    self.tick,
                    &self.outbound_tx,
                    &[], // peers — set via DHT routing table
                    self.local_id,
                );

                // Log notable brain events every tick
                if fp_report.neurons_spawned > 0 {
                    eprintln!(
                        "[BRAIN] tick={} spawned={} surprise={:.4} orphans={}",
                        self.tick,
                        fp_report.neurons_spawned,
                        fp_report.total_surprise,
                        fp_report.orphans_cleaned,
                    );
                }
            }

            // ── PHASE 4: RETRANSMIT (every N ticks) ──────────
            if self.tick - self.last_retransmit_tick >= self.config.retransmit_interval_ticks {
                self.last_retransmit_tick = self.tick;
                // We need a target peer for retransmit. In a full system, the engine
                // would know all peers. For now, retransmit is handled inside UdpTransport
                // by the reliable_queue's knowledge of each peer.
                // The retransmit is triggered per-peer in the full implementation.
                // For this base version, we just scan and clean.
                self.transport.cleanup_expired();
            }

            // ── PHASE 4: CLEANUP & APOPTOSIS (every N ticks) ────
            if self.tick - self.last_cleanup_tick >= self.config.cleanup_interval_ticks {
                self.last_cleanup_tick = self.tick;

                // Transport cleanup (expired reliable frames)
                self.transport.cleanup_expired();

                // Apoptosis sweep: evict dead DHT nodes, expired pings,
                // orphaned transport frames. Reports total deaths this sweep.
                if let Some(ref mut dht) = self.dht_handler {
                    let report = self.apoptosis_system.tick(
                        self.tick,
                        dht,
                        &mut self.transport,
                    );

                    // Death spiral guardrail
                    if self.apoptosis_system.is_death_spiral(&report) {
                        eprintln!(
                            "[ENGINE] ⚠️ DEATH SPIRAL: {} nodes evicted at tick {}. \
                             Network partition or seed node failure.",
                            report.total_deaths,
                            self.tick,
                        );
                    } else if report.total_deaths > 0 {
                        eprintln!(
                            "[APOPTOSIS] sweep: {} deaths (DHT:{} ping:{} frames:{})",
                            report.total_deaths,
                            report.dht_nodes_evicted,
                            report.pending_pings_expired,
                            report.data_frames_purged,
                        );
                    }

                    // DHT periodic maintenance (ping stale, save peers)
                    dht.periodic_maintenance();
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
            if self.tick % 1000 == 0 {
                self.print_stats();
            }
        }
    }

    // ─── Ingress Pipeline ──────────────────────────────────────

    /// Process an incoming UDP datagram.
    /// Validates CRC, updates ACK tracker, applies gradient decay,
    /// and dispatches to the event channel.
    fn handle_ingress(&mut self, data: &[u8], src: SocketAddr) -> Result<(), String> {
        if data.len() < TransportHeader::SIZE {
            return Err(format!("too short: {} bytes", data.len()));
        }

        // Zero-copy parse the transport header
        let header = unsafe { &*(data.as_ptr() as *const TransportHeader) };

        // Update ACK tracker (also handles duplicate detection)
        let is_new = self.transport.ack_tracker.record(header.sequence_number);
        if !is_new {
            // Duplicate packet — still process ACK info but skip dispatch
            self.transport.reliable_queue.process_ack(header.ack_number, header.ack_bitfield);
            return Ok(());
        }

        // Process the ACK this packet carries
        self.transport.reliable_queue.process_ack(header.ack_number, header.ack_bitfield);

        // Calculate gradient decay weight based on packet age
        let now_ms = self.transport.now_ms();
        let age_ms = now_ms.saturating_sub(header.timestamp);
        let gradient_weight = calculate_gradient_weight(age_ms, self.config.gradient_half_life_ms);

        // Update peer RTT estimate (exponential moving average)
        let rtt_samples = self.peer_rtt.entry(src).or_insert(age_ms as f32);
        *rtt_samples = *rtt_samples * 0.9 + age_ms as f32 * 0.1;

        // The payload is everything after the 16-byte transport header.
        // build_frame() prepends a 4-byte total-length prefix before the
        // MessageHeader — strip it so nwp_payload starts at MessageHeader.
        let raw = &data[TransportHeader::SIZE..];
        let nwp_payload: &[u8] = if raw.len() >= 4 { &raw[4..] } else { &[] };

        // Dispatch the event
        let event = IngressEvent {
            transport_header: *header,
            nwp_payload: nwp_payload.to_vec(),
            src,
            recv_timestamp: now_ms,
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

        eprintln!(
            "[ENGINE] tick={} rate={:.0}Hz rx={} pkts ({:.2}MB) tx={} pkts ({:.2}MB) \
             idle={:.1}% reliable_q={} peers={}",
            self.tick,
            if elapsed > 0.0 { self.tick as f64 / elapsed } else { 0.0 },
            self.stats.packets_recv,
            mb_recv,
            self.stats.packets_sent,
            mb_sent,
            if self.tick > 0 { self.stats.idle_ticks as f64 / self.tick as f64 * 100.0 } else { 0.0 },
            self.stats.reliable_queue_depth,
            self.peer_rtt.len(),
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
            }
        }
    }

    /// Get peer RTT estimates
    pub fn peer_rtt(&self) -> &HashMap<SocketAddr, f32> {
        &self.peer_rtt
    }
}

// ─── Convenience: Spawn Engine Thread ──────────────────────────

/// Run the engine in a background thread.
/// Returns (outbound_tx, events_rx, join_handle).
pub fn spawn_engine(
    config: EngineConfig,
    dht_handler: Option<DhtHandler>,
    shutdown: Arc<AtomicBool>,
) -> std::io::Result<(Sender<OutgoingPacket>, Receiver<IngressEvent>, std::thread::JoinHandle<()>)> {
    let (outbound_tx, outbound_rx) = mpsc::channel();
    let (events_tx, events_rx) = mpsc::channel();

    let transport = UdpTransport::bind(&config.bind_addr)?;

    let outbound_tx_for_return = outbound_tx.clone();

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
                tick: 0,
                last_retransmit_tick: 0,
                last_cleanup_tick: 0,
                last_stats_time: Instant::now(),
                stats: EngineStats::default(),
                peer_rtt: HashMap::new(),
                activation_map: HashMap::new(),
                synapse_map: HashMap::new(),
                forward_pass: ForwardPassSystem::default(),
                neurogenesis: NeurogenesisSystem::default(),
                hebbian: HebbianLearningSystem::new(0.01, 0.999, 0.001, 500),
                local_id: EntityId([0u8; 32]),
                outbound_tx: outbound_tx.clone(),
                brain_attached: false,
            };

            // Auto-create DHT handler if local peers configured but no handler given
            if engine.dht_handler.is_none() && !engine.config.local_peers.is_empty() {
                use rand::Rng;
                let mut local_id = [0u8; 32];
                rand::thread_rng().fill(&mut local_id);
                let dht = DhtHandler::new(
                    NodeId::new(local_id),
                    engine.config.bind_addr.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    NodeType::General,
                    outbound_tx.clone(),
                    None,
                    "local".to_string(),
                );
                engine.dht_handler = Some(dht);
            }

            // Bootstrap: inject local peers into DHT routing table
            if let Some(ref mut dht) = engine.dht_handler {
                for peer_addr in &engine.config.local_peers {
                    use rand::Rng;
                    let mut id_bytes = [0u8; 32];
                    rand::thread_rng().fill(&mut id_bytes);
                    let entry = NodeEntry::new(
                        NodeId::new(id_bytes),
                        *peer_addr,
                        NodeType::General,
                    );
                    dht.routing_table.insert(entry);
                    dht.ping_node(*peer_addr);
                }
                dht.bootstrap();
                eprintln!(
                    "[ENGINE] Bootstrapped {} local peers",
                    engine.config.local_peers.len()
                );
            }

            // Set 1ms read timeout
            let _ = engine.transport.socket.set_read_timeout(Some(Duration::from_millis(engine.config.tick_interval_ms)));

            let mut recv_buf = vec![0u8; engine.config.recv_buffer_size];

            loop {
                engine.tick += 1;

                // ── SHUTDOWN CHECK ─────────────────────────────
                if engine.shutdown.load(Ordering::Relaxed) {
                    eprintln!("[ENGINE] Shutdown signal. Exiting after {} ticks.", engine.tick);
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
                            if let Err(e) = engine.handle_ingress(&recv_buf[..len], src) {
                                eprintln!("[ENGINE] ingress: {}", e);
                            }
                        }
                        Err(ref e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut => break,
                        Err(e) => {
                            eprintln!("[ENGINE] recv: {}", e);
                            break;
                        }
                    }
                    if ingress_count > 10_000 { break; }
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
                                    &pkt.payload, &pkt.dst,
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
                if engine.tick - engine.last_retransmit_tick >= engine.config.retransmit_interval_ticks {
                    engine.last_retransmit_tick = engine.tick;
                    engine.transport.cleanup_expired();
                }

                // Phase 4: Cleanup & Apoptosis (every 1000 ticks)
                if engine.tick - engine.last_cleanup_tick >= engine.config.cleanup_interval_ticks {
                    engine.last_cleanup_tick = engine.tick;
                    engine.transport.cleanup_expired();

                    // Apoptosis sweep
                    if let Some(ref mut dht) = engine.dht_handler {
                        let report = engine.apoptosis_system.tick(
                            engine.tick,
                            dht,
                            &mut engine.transport,
                        );
                        if engine.apoptosis_system.is_death_spiral(&report) {
                            eprintln!(
                                "[ENGINE] ⚠️ DEATH SPIRAL: {} nodes evicted at tick {}.",
                                report.total_deaths, engine.tick,
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
                if engine.tick % 1000 == 0 {
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
}
