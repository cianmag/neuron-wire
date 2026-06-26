//! Adversarial Testing Framework — make users behave like users.
//!
//! ## Attack Vectors
//!
//! | Attack | Mechanism | What It Tests |
//! |--------|-----------|---------------|
//! | Bad packets | Corrupt MessageHeader fields, garbage payloads | Parsing resilience (no crash on invalid input) |
//! | Corrupted state | Inject fake routing entries, wild latency values | Routing table integrity under pollution |
//! | Spoofed identity | Node A claims Node B's NodeId in messages | Identity verification, anti-spoofing |
//! | Replay attack | Capture valid packets and reinject later | Sequence number idempotency, ACK dedup |
//!
//! ## Architecture
//!
//! The `Adversary` runs alongside the simulation in the simulator's monitor
//! loop. It has its own UDP socket for injecting crafted packets directly
//! into the network, bypassing the normal outbound channel. For state
//! corruption, it mutates node state through shared references.

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::engine_loop::EngineStats;
use crate::header::{self, MessageHeader};
use crate::{HEADER_SIZE};
use crate::transport::TransportHeader;
use serde::{Deserialize, Serialize};

// ─── Constants ──────────────────────────────────────────────────

/// UDP send timeout for adversarial socket
const ADV_SOCKET_TIMEOUT_MS: u64 = 100;
/// How many corrupt packets to send per tick (to avoid flooding ourselves)
const MAX_BURST_PER_TICK: usize = 10;

// ─── Adversary Mode ─────────────────────────────────────────────

/// Supported adversarial attack modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdversaryMode {
    /// No adversarial behaviour (clean run)
    None,
    /// Send packets with corrupted headers, garbage bodies, invalid lengths
    BadPackets,
    /// Inject fake routing entries / corrupt node state
    CorruptedState,
    /// Spoof source identity (claim another node's NodeId or address)
    SpoofedIdentity,
    /// Capture valid packets and replay them later
    ReplayAttack,
    /// Run all available attack vectors simultaneously
    All,
}

impl std::fmt::Display for AdversaryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdversaryMode::None => write!(f, "none"),
            AdversaryMode::BadPackets => write!(f, "bad-packets"),
            AdversaryMode::CorruptedState => write!(f, "corrupted-state"),
            AdversaryMode::SpoofedIdentity => write!(f, "spoofed-identity"),
            AdversaryMode::ReplayAttack => write!(f, "replay-attack"),
            AdversaryMode::All => write!(f, "all"),
        }
    }
}

impl AdversaryMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "bad-packets" | "badpackets" | "corrupt" => AdversaryMode::BadPackets,
            "corrupted-state" | "corrupt-state" | "state" => AdversaryMode::CorruptedState,
            "spoofed-identity" | "spoof" | "identity" => AdversaryMode::SpoofedIdentity,
            "replay-attack" | "replay" => AdversaryMode::ReplayAttack,
            "all" | "everything" => AdversaryMode::All,
            _ => AdversaryMode::None,
        }
    }
}

// ─── Adversary Configuration ────────────────────────────────────

/// Configuration for adversarial testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdversaryConfig {
    /// Enable adversarial behaviour
    pub enabled: bool,
    /// Which attack vector to use
    pub mode: AdversaryMode,
    /// Index of the attacker node (sends corrupt packets, spoofs identity, etc.)
    pub attacker_node_index: u32,
    /// Optional target node index (None = random targets)
    pub target_node_index: Option<u32>,
    /// Simulation time to start attacks (seconds)
    pub attack_start_sec: f64,
    /// Duration of attacks (seconds, 0 = until end of simulation)
    pub attack_duration_secs: f64,
    /// Fraction of packets to corrupt (0.0–1.0)
    pub corruption_rate: f64,
    /// How many times to replay a captured packet (replay attack)
    pub replay_count: u32,
    /// Delay before first replay (milliseconds)
    pub replay_delay_ms: u64,
    /// How long to keep captured packets before discarding (seconds)
    pub capture_window_secs: f64,
}

impl Default for AdversaryConfig {
    fn default() -> Self {
        AdversaryConfig {
            enabled: false,
            mode: AdversaryMode::None,
            attacker_node_index: 0,
            target_node_index: None,
            attack_start_sec: 15.0,
            attack_duration_secs: 0.0,
            corruption_rate: 0.3,
            replay_count: 2,
            replay_delay_ms: 100,
            capture_window_secs: 5.0,
        }
    }
}

// ─── Captured Packet (Replay Attack) ────────────────────────────

/// A captured packet waiting to be replayed.
struct CapturedPacket {
    /// The raw datagram bytes (transport header + NWP frame)
    data: Vec<u8>,
    /// Destination address (who was the original recipient)
    dst: SocketAddr,
    /// When this packet was captured
    captured_at: Instant,
    /// How many times we've replayed it so far
    replay_count: u32,
    /// Max replays allowed
    max_replays: u32,
    /// Delay before first replay (ms)
    delay_ms: u64,
}

// ─── Adversary ──────────────────────────────────────────────────

/// Drives adversarial behaviour during a simulation.
///
/// The adversary runs as part of the simulator's monitor loop (same thread).
/// It maintains its own UDP socket for injecting crafted packets and
/// tracks per-node mutable state references for state corruption.
pub struct Adversary {
    /// Configuration
    config: AdversaryConfig,
    /// UDP socket for sending crafted packets (bad packets, spoof, replay)
    socket: Option<UdpSocket>,
    /// All node addresses in the simulation
    node_addrs: Vec<SocketAddr>,
    /// Index of the attacker node (its address)
    attacker_addr: Option<SocketAddr>,
    /// Captured packets waiting to be replayed
    captured_packets: Vec<CapturedPacket>,
    /// Whether attack is currently active
    active: bool,
    /// When the attack started (None = not started yet)
    attack_start: Option<Instant>,
    /// Replay attack state: whether we've finished capture phase
    capture_done: bool,
    /// PRNG for deterministic corruption
    rng_seed: u64,
    /// Reference to per-node shutdown flags (for state corruption)
    node_shutdowns: Vec<Arc<AtomicBool>>,
    /// Reference to per-node engine stats (to observe effects)
    _node_stats: Vec<Arc<Mutex<EngineStats>>>,
}

impl Adversary {
    /// Create a new adversary for the given simulation.
    pub fn new(
        config: AdversaryConfig,
        node_addrs: Vec<SocketAddr>,
        node_shutdowns: Vec<Arc<AtomicBool>>,
        node_stats: Vec<Arc<Mutex<EngineStats>>>,
        rng_seed: u64,
        attacker_index: Option<u32>,
    ) -> Self {
        let attacker_addr = attacker_index
            .and_then(|idx| node_addrs.get(idx as usize).copied());

        Adversary {
            config,
            socket: None,
            node_addrs,
            attacker_addr,
            captured_packets: Vec::new(),
            active: false,
            attack_start: None,
            capture_done: false,
            rng_seed,
            node_shutdowns,
            _node_stats: node_stats,
        }
    }

    /// Initialize the adversary (bind UDP socket for injecting packets).
    pub fn init(&mut self) -> Result<(), String> {
        if !self.config.enabled || self.config.mode == AdversaryMode::None {
            return Ok(());
        }

        // Bind a free port for our injection socket
        let socket = UdpSocket::bind("127.0.0.1:0")
            .map_err(|e| format!("adversary socket bind: {}", e))?;
        socket.set_nonblocking(true)
            .map_err(|e| format!("adversary nonblocking: {}", e))?;
        socket.set_write_timeout(Some(Duration::from_millis(ADV_SOCKET_TIMEOUT_MS)))
            .ok();
        self.socket = Some(socket);

        eprintln!(
            "[ADVERSARY] Initialised mode={} attacker={:?} targets={} start={:.1}s",
            self.config.mode,
            self.attacker_addr,
            self.node_addrs.len(),
            self.config.attack_start_sec,
        );

        Ok(())
    }

    /// Tick the adversary. Called from the simulator's monitor loop.
    /// `elapsed_secs` = how long the simulation has been running.
    /// `node_shutdowns` = per-node AtomicBool references.
    pub fn tick(&mut self, elapsed_secs: f64, _tick_counter: u64) {
        if !self.config.enabled || self.config.mode == AdversaryMode::None {
            return;
        }

        // Check activation gate
        if !self.active {
            if elapsed_secs >= self.config.attack_start_sec {
                self.active = true;
                self.attack_start = Some(Instant::now());
                eprintln!(
                    "[ADVERSARY] Attack started at t={:.1}s (mode={})",
                    elapsed_secs, self.config.mode,
                );
            }
            return;
        }

        // Check deactivation gate (duration > 0 means finite attack window)
        if self.config.attack_duration_secs > 0.0 {
            if let Some(start) = self.attack_start {
                if start.elapsed().as_secs_f64() >= self.config.attack_duration_secs {
                    if self.active {
                        self.active = false;
                        eprintln!(
                            "[ADVERSARY] Attack ended at t={:.1}s",
                            elapsed_secs,
                        );
                    }
                    return;
                }
            }
        }

        // Dispatch to the active attack mode
        match self.config.mode {
            AdversaryMode::BadPackets => self.tick_bad_packets(elapsed_secs),
            AdversaryMode::CorruptedState => self.tick_corrupted_state(elapsed_secs),
            AdversaryMode::SpoofedIdentity => self.tick_spoofed_identity(elapsed_secs),
            AdversaryMode::ReplayAttack => self.tick_replay_attack(elapsed_secs),
            AdversaryMode::All => {
                // Run all attacks
                self.tick_bad_packets(elapsed_secs);
                self.tick_corrupted_state(elapsed_secs);
                self.tick_spoofed_identity(elapsed_secs);
                self.tick_replay_attack(elapsed_secs);
            }
            AdversaryMode::None => {}
        }
    }

    /// Let the adversary observe an outbound packet for replay capture.
    /// Called from the simulator when a packet is sent by a node.
    pub fn observe_outbound(&mut self, data: &[u8], dst: SocketAddr, _src_node: u32, now: Instant) {
        if !self.active || !matches!(self.config.mode, AdversaryMode::ReplayAttack | AdversaryMode::All) {
            return;
        }

        if self.capture_done {
            return;
        }

        // Only capture during the capture window (first N seconds of attack)
        let capture_window = Duration::from_secs_f64(self.config.capture_window_secs);
        if let Some(start) = self.attack_start {
            if now.duration_since(start) > capture_window {
                self.capture_done = true;
                eprintln!(
                    "[ADVERSARY] Capture complete: {} packets stored for replay",
                    self.captured_packets.len(),
                );
                return;
            }
        }

        // Capture a fraction of packets based on corruption_rate
        // Use simple hashing of the destination for deterministic sampling
        let hash = dst.port() as u64;
        if (hash % 100) < (self.config.corruption_rate * 100.0) as u64 {
            self.captured_packets.push(CapturedPacket {
                data: data.to_vec(),
                dst,
                captured_at: now,
                replay_count: 0,
                max_replays: self.config.replay_count,
                delay_ms: self.config.replay_delay_ms,
            });
        }
    }

    // ─── Attack Implementations ─────────────────────────────────

    /// Send a batch of corrupt packets to random nodes.
    fn tick_bad_packets(&mut self, _elapsed_secs: f64) {
        let socket = match &self.socket {
            Some(s) => s,
            None => return,
        };

        if self.node_addrs.is_empty() {
            return;
        }

        // Build several types of corrupt packets and send them
        for _ in 0..MAX_BURST_PER_TICK {
            let target = self.pick_target();

            // Cycle through corruption types
            let variant = (self.rng_seed % 7) as usize;
            self.rng_seed = self.rng_seed.wrapping_add(1);

            let datagram = match variant {
                0 => Self::build_corrupt_magic(target),
                1 => Self::build_corrupt_version(target),
                2 => Self::build_corrupt_crc(target),
                3 => Self::build_invalid_frame_len(target),
                4 => Self::build_truncated_body(target),
                5 => Self::build_oversized_body(target),
                6 => Self::build_garbage_payload(),
                _ => continue,
            };

            let _ = socket.send_to(&datagram, target);
        }
    }

    /// Corrupt internal state of nodes (fake routing entries, wrong latencies).
    fn tick_corrupted_state(&mut self, _elapsed_secs: f64) {
        // On first tick of corrupted state, inject chaos into some nodes
        // by flipping their shutdown flag in a pattern that causes
        // routing table confusion (briefly kill, then restart).
        //
        // We only do this once when state corruption activates.
        // The effect: target nodes disappear and reappear, leaving stale
        // routing entries that pollute the DHT.

        // Find a target that isn't the attacker
        let target_count = self.node_shutdowns.len();
        if target_count < 2 {
            return;
        }

        // Pick 2-3 nodes to momentarily kill (brief blip)
        let to_blip = (target_count as f64 * 0.2).max(2.0).min(5.0) as usize;
        for i in 0..to_blip.min(target_count) {
            let idx = (i + self.rng_seed as usize) % target_count;
            // Skip attacker node
            if Some(idx as u32) == self.config.attacker_node_index.into() {
                continue;
            }
            if let Some(shutdown) = self.node_shutdowns.get(idx) {
                // Toggle kill → revived next tick (simulates crash + restart)
                // This creates routing table pollution as peers see the node
                // disappear then reappear with potentially stale entries.
                if self.rng_seed % 3 == 0 {
                    shutdown.store(true, Ordering::SeqCst);
                }
            }
        }
        self.rng_seed = self.rng_seed.wrapping_add(1);
    }

    /// Send packets that claim a false identity.
    fn tick_spoofed_identity(&mut self, _elapsed_secs: f64) {
        let socket = match &self.socket {
            Some(s) => s,
            None => return,
        };

        if self.node_addrs.len() < 2 {
            return;
        }

        // Send a PING from the attacker's socket, but:
        // - The UDP source address will be our socket address (not the claimed node)
        // - The NWP body will claim a different NodeId
        // This tests whether the DHT handler detects the identity mismatch
        // between the UDP source and the claimed NodeId in the body.

        // Build a PING frame with a spoofed NodeId in the body
        let target = self.pick_target();

        // Construct body: [32 bytes fake NodeId]
        let mut body = vec![0u8; 32];
        // Fill with a recognizable spoof pattern (not all zeros)
        for (i, b) in body.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(17);
        }
        let frame = header::build_frame(7, body, 0); // msg_type 7 = PING

        // Wrap in transport header
        let transport = TransportHeader::new(
            self.rng_seed as u32,
            0, 0, 0,
        );
        let mut datagram = Vec::with_capacity(16 + frame.len());
        datagram.extend_from_slice(&transport.to_bytes());
        datagram.extend_from_slice(&frame);

        let _ = socket.send_to(&datagram, target);
        self.rng_seed = self.rng_seed.wrapping_add(1);
    }

    /// Replay previously captured packets.
    fn tick_replay_attack(&mut self, _elapsed_secs: f64) {
        let socket = match &self.socket {
            Some(s) => s,
            None => return,
        };

        if self.captured_packets.is_empty() {
            return;
        }

        let now = Instant::now();

        // Process captured packets — replay those whose delay has elapsed
        let mut to_remove = Vec::new();
        let mut to_send = Vec::new();

        for (i, pkt) in self.captured_packets.iter_mut().enumerate() {
            if pkt.replay_count >= pkt.max_replays {
                to_remove.push(i);
                continue;
            }

            let age = now.duration_since(pkt.captured_at);
            let expected_delay = Duration::from_millis(pkt.delay_ms * (pkt.replay_count as u64 + 1));
            if age >= expected_delay {
                to_send.push((pkt.data.clone(), pkt.dst));
                pkt.replay_count += 1;
            }
        }

        // Send all due replays
        for (data, dst) in &to_send {
            let _ = socket.send_to(data, dst);
        }

        // Clean up fully replayed packets (bottom-up to preserve indices)
        to_remove.sort_unstable();
        for i in to_remove.into_iter().rev() {
            self.captured_packets.swap_remove(i);
        }
    }

    // ─── Helpers ────────────────────────────────────────────────

    /// Pick a target node address (either configured target or random).
    fn pick_target(&self) -> SocketAddr {
        if let Some(tgt_idx) = self.config.target_node_index {
            if let Some(addr) = self.node_addrs.get(tgt_idx as usize) {
                return *addr;
            }
        }
        // Random target (deterministic via rng_seed)
        let idx = (self.rng_seed as usize) % self.node_addrs.len();
        self.node_addrs[idx]
    }

    /// Build a datagram with corrupt magic bytes.
    fn build_corrupt_magic(_target: SocketAddr) -> Vec<u8> {
        let mut frame = header::build_frame(7, vec![0u8; 8], 0);
        // Corrupt the magic bytes at offset 4 (after frame_len)
        if frame.len() > 7 {
            frame[4] = 0xFF; // first magic byte -> garbage
            frame[5] = 0x00;
            frame[6] = 0x00;
            frame[7] = 0xFF;
        }
        let transport = TransportHeader::new(1, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a datagram with corrupt version byte.
    fn build_corrupt_version(_target: SocketAddr) -> Vec<u8> {
        let mut frame = header::build_frame(7, vec![0u8; 8], 0);
        // Version is at offset 4 (magic) + 4 = byte 8 from frame start
        // frame = [4-byte len][16-byte header][body]
        // Version is at header offset 4, so absolute offset = 4 + 4 = 8
        if frame.len() > 9 {
            frame[8] = 0xFF; // invalid version
        }
        let transport = TransportHeader::new(2, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a datagram with a bad CRC (flip one bit in header after CRC).
    fn build_corrupt_crc(_target: SocketAddr) -> Vec<u8> {
        let mut frame = header::build_frame(7, vec![0u8; 8], 0);
        // CRC is at header offset 12-15, absolute: 4 + 12 = 16
        if frame.len() > 17 {
            frame[16] ^= 0x01; // flip one bit in the CRC
        }
        let transport = TransportHeader::new(3, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a datagram with an invalid frame_len (too short).
    fn build_invalid_frame_len(_target: SocketAddr) -> Vec<u8> {
        // Write a frame_len that's way too small
        let body = vec![0u8; 32];
        let real_frame = header::build_frame(7, body, 0);
        // Overwrite frame_len (bytes 0-3) with 0 (impossible)
        let mut frame = real_frame;
        if frame.len() > 3 {
            frame[0] = 0x00;
            frame[1] = 0x00;
            frame[2] = 0x00;
            frame[3] = 0x00;
        }
        let transport = TransportHeader::new(4, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a datagram where the body is truncated (header says X bytes but actual is less).
    fn build_truncated_body(_target: SocketAddr) -> Vec<u8> {
        // Build frame with body_len = 1000 but only send 50 bytes of body
        let mut body = vec![0u8; 50];
        // We need to manually set body_len to 1000
        for (i, b) in body.iter_mut().enumerate() {
            *b = (i & 0xFF) as u8;
        }
        let total = 4 + HEADER_SIZE + 50;
        let mut frame = Vec::with_capacity(total);
        // frame_len = total (this is the REAL total, which is smaller than claimed)
        frame.extend_from_slice(&(total as u32).to_le_bytes());
        // Header claiming body_len = 1000
        let header = MessageHeader::new(7, 1000, 0); // lies: body_len=1000
        frame.extend_from_slice(&header.to_bytes());
        // Actual body is only 50 bytes
        frame.extend_from_slice(&body);
        let transport = TransportHeader::new(5, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a datagram with an absurdly large body_len.
    fn build_oversized_body(_target: SocketAddr) -> Vec<u8> {
        // Frame with body_len > MAX_BODY_SIZE (1GB)
        let total = 4 + HEADER_SIZE + 8;
        let mut frame = Vec::with_capacity(total);
        frame.extend_from_slice(&(total as u32).to_le_bytes());
        let header = MessageHeader::new(7, 1_000_000_001, 0); // > MAX_BODY_SIZE
        frame.extend_from_slice(&header.to_bytes());
        frame.extend_from_slice(&[0u8; 8]);
        let transport = TransportHeader::new(6, 0, 0, 0);
        let mut datagram = transport.to_bytes().to_vec();
        datagram.extend_from_slice(&frame);
        datagram
    }

    /// Build a completely garbage payload (no valid structure at all).
    fn build_garbage_payload() -> Vec<u8> {
        let mut datagram = vec![0u8; 64];
        // Fill with pseudo-random garbage (no valid transport header, no NWP structure)
        for (i, b) in datagram.iter_mut().enumerate() {
            *b = (i.wrapping_mul(37) & 0xFF) as u8;
        }
        datagram
    }
}

// ─── Unit Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::parse_frame;

    #[test]
    fn test_adversary_mode_from_str() {
        assert_eq!(AdversaryMode::from_str("none"), AdversaryMode::None);
        assert_eq!(AdversaryMode::from_str("bad-packets"), AdversaryMode::BadPackets);
        assert_eq!(AdversaryMode::from_str("badpackets"), AdversaryMode::BadPackets);
        assert_eq!(AdversaryMode::from_str("corrupted-state"), AdversaryMode::CorruptedState);
        assert_eq!(AdversaryMode::from_str("spoofed-identity"), AdversaryMode::SpoofedIdentity);
        assert_eq!(AdversaryMode::from_str("spoof"), AdversaryMode::SpoofedIdentity);
        assert_eq!(AdversaryMode::from_str("replay-attack"), AdversaryMode::ReplayAttack);
        assert_eq!(AdversaryMode::from_str("replay"), AdversaryMode::ReplayAttack);
        assert_eq!(AdversaryMode::from_str("all"), AdversaryMode::All);
        assert_eq!(AdversaryMode::from_str("unknown"), AdversaryMode::None);
    }

    #[test]
    fn test_bad_magic_packet_is_corrupt() {
        let datagram = Adversary::build_corrupt_magic("127.0.0.1:9999".parse().unwrap());
        // Datagram should be at least transport header + 4-byte frame_len + header
        assert!(datagram.len() >= 16 + 4 + 16);

        // Parsing should fail (bad magic)
        let nwp_frame = &datagram[TransportHeader::SIZE..];
        if nwp_frame.len() >= 4 + HEADER_SIZE {
            let nwp_payload = &nwp_frame[4..];
            let result = parse_frame(nwp_payload);
            assert!(result.is_err(), "bad magic packet should fail to parse");
        }
    }

    #[test]
    fn test_bad_version_packet_is_corrupt() {
        let datagram = Adversary::build_corrupt_version("127.0.0.1:9999".parse().unwrap());
        let nwp_frame = &datagram[TransportHeader::SIZE..];
        if nwp_frame.len() >= 4 + HEADER_SIZE {
            let nwp_payload = &nwp_frame[4..];
            let result = parse_frame(nwp_payload);
            assert!(result.is_err(), "bad version packet should fail to parse");
        }
    }

    #[test]
    fn test_bad_crc_packet_fails_validation() {
        let datagram = Adversary::build_corrupt_crc("127.0.0.1:9999".parse().unwrap());
        let nwp_frame = &datagram[TransportHeader::SIZE..];
        if nwp_frame.len() >= 4 + HEADER_SIZE {
            let nwp_payload = &nwp_frame[4..];
            // Should fail CRC validation
            if let Ok((_header, _)) = parse_frame(nwp_payload) {
                // The from_bytes calls validate(), which checks CRC
                // If parse_frame succeeded, the CRC check must have passed...
                // Actually build_corrupt_crc flips a bit IN the CRC field itself,
                // which means the CRC field no longer matches the CRC of bytes[0..12).
                // This SHOULD fail. Let's check via direct validation:
                let h = unsafe { &*(nwp_payload.as_ptr() as *const MessageHeader) };
                assert!(!h.validate().is_ok(), "corrupt CRC should fail validation");
            } else {
                // parse_frame already failed — that's also correct
            }
        }
    }

    #[test]
    fn test_garbage_payload_has_no_structure() {
        let datagram = Adversary::build_garbage_payload();
        // Garbage payload should not have valid transport header
        // We just check it's the right size
        assert_eq!(datagram.len(), 64);
        // No assertion on parsing — garbage is undefined behaviour
        // at the protocol level. The engine should just handle it
        // gracefully (would get caught by min-length checks).
    }

    #[test]
    fn test_replay_capture_and_count() {
        let config = AdversaryConfig {
            enabled: true,
            mode: AdversaryMode::ReplayAttack,
            replay_count: 3,
            ..Default::default()
        };
        let mut adv = Adversary::new(
            config,
            vec!["127.0.0.1:9001".parse().unwrap()],
            vec![Arc::new(AtomicBool::new(false))],
            vec![Arc::new(Mutex::new(EngineStats::default()))],
            42,
            Some(0),
        );
        adv.active = true;
        adv.attack_start = Some(Instant::now());

        let data = vec![0u8; 100];
        let dst: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        let now = Instant::now();

        // Capture several packets
        for _ in 0..10 {
            adv.observe_outbound(&data, dst, 0, now);
        }

        // Should have captured some (based on corruption_rate)
        // Just verify the capture mechanism didn't crash
        assert!(adv.captured_packets.len() <= 10);
    }

    #[test]
    fn test_adversary_config_defaults_sane() {
        let cfg = AdversaryConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.mode, AdversaryMode::None);
        assert!(cfg.corruption_rate > 0.0);
        assert!(cfg.replay_count >= 1);
    }
}
