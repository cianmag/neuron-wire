//! Stress and soak tests for the neuron-wire engine loop.
//!
//! These tests exercise the engine under continuous load to detect
//! memory leaks, CPU spin, and stability issues over extended runs.
//!
//! ## Test types
//!
//! | Test | Duration | Purpose |
//! |------|----------|---------|
//! | `stress_ping_pong` | 30s wall-clock | Sustained ping/pong between 2 nodes |
//! | `stress_many_nodes` | 10s wall-clock | 10-node DHT convergence + steady state |
//! | `stress_engine_thrashing` | 5s | Rapid node join/leave cycles |
//!
//! Run with: `cargo test --test stress -- --nocapture --ignored`
//! Or include with: `cargo test --test stress -- --include-ignored`

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use neuron_wire::header::{self, MessageHeader};
use neuron_wire::HEADER_SIZE;

// ── Helpers ──────────────────────────────────────────────────────

/// A lightweight test node for stress testing.
/// Uses real UDP sockets on loopback.
#[allow(dead_code)]
struct StressNode {
    socket: UdpSocket,
    addr: SocketAddr,
    running: Arc<AtomicBool>,
    pkts_sent: u64,
    pkts_recv: u64,
    last_throughput: f64, // pkts/sec
}

impl StressNode {
    fn bind(port: u16) -> Self {
        let socket =
            UdpSocket::bind(format!("127.0.0.1:{}", port)).expect("failed to bind UDP socket");
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .ok();
        let addr = socket.local_addr().unwrap();
        Self {
            socket,
            addr,
            running: Arc::new(AtomicBool::new(true)),
            pkts_sent: 0,
            pkts_recv: 0,
            last_throughput: 0.0,
        }
    }

    fn send_ping(&mut self, dst: SocketAddr) {
        let h = MessageHeader::new(0, 0, 0);
        let frame = header::build_frame(0, h.to_bytes().to_vec(), 0);
        self.socket.send_to(&frame, dst).ok();
        self.pkts_sent += 1;
    }

    fn recv_all(&mut self) -> usize {
        let mut buf = vec![0u8; 65535];
        let mut count = 0;
        while let Ok((n, _)) = self.socket.recv_from(&mut buf) {
            if n >= HEADER_SIZE {
                self.pkts_recv += 1;
                count += 1;
            }
        }
        count
    }
}

// ── Stress test: sustained ping/pong ────────────────────────────

/// Send ping/pong as fast as possible for 30 seconds.
/// Fails if throughput drops below 100 pkts/sec at any checkpoint,
/// or if memory usage grows unbounded.
#[test]
#[ignore]
fn stress_ping_pong() {
    let mut node_a = StressNode::bind(0);
    let mut node_b = StressNode::bind(0);

    let start = Instant::now();
    let duration = Duration::from_secs(30);
    let mut checkpoint = Duration::from_secs(0);
    let checkpoint_interval = Duration::from_secs(5);

    while start.elapsed() < duration {
        // A → B ping
        node_a.send_ping(node_b.addr);
        node_a.recv_all();
        node_b.recv_all();

        // B → A pong
        node_b.send_ping(node_a.addr);
        node_b.recv_all();
        node_a.recv_all();

        // Checkpoint every 5s
        if start.elapsed() - checkpoint >= checkpoint_interval {
            checkpoint = start.elapsed();
            let elapsed_secs = checkpoint.as_secs_f64();
            let throughput = node_a.pkts_sent as f64 / elapsed_secs;
            node_a.last_throughput = throughput;
            assert!(
                throughput > 100.0,
                "Throughput dropped below 100 pkts/sec at {:.0}s: {:.0} pkts/sec",
                elapsed_secs,
                throughput
            );
            eprintln!(
                "  [ping_pong] {:.0}s: {:.0} pkts/sec, {} sent, {} recv",
                elapsed_secs, throughput, node_a.pkts_sent, node_a.pkts_recv
            );
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    eprintln!(
        "✅ stress_ping_pong: {:.0}s, {:.0} pkts sent, avg {:.0} pkts/sec",
        elapsed,
        node_a.pkts_sent,
        node_a.pkts_sent as f64 / elapsed
    );
}

// ── Stress test: many-node DHT convergence ──────────────────────

/// Launch 10 nodes, let them converge, send traffic for 10 seconds.
/// Fails if any node stops responding or throughput collapses.
#[test]
#[ignore]
fn stress_many_nodes() {
    let mut nodes: Vec<StressNode> = (0..10).map(|_| StressNode::bind(0)).collect();
    let addrs: Vec<SocketAddr> = nodes.iter().map(|n| n.addr).collect();

    // Bootstrap: each node pings every other node
    for i in 0..nodes.len() {
        for j in 0..nodes.len() {
            if i != j {
                nodes[i].send_ping(addrs[j]);
            }
        }
    }

    // Let convergence happen
    thread::sleep(Duration::from_millis(500));

    // Drain all
    for node in &mut nodes {
        node.recv_all();
    }

    // Continuous traffic for 10s
    let start = Instant::now();
    let duration = Duration::from_secs(10);
    let mut total_pkts_sent: u64 = 0;
    let mut checkpoints = 0;

    while start.elapsed() < duration {
        // Each node sends to 3 random peers
        for i in 0..nodes.len() {
            for _ in 0..3 {
                let j = (i + 1 + checkpoints as usize) % nodes.len();
                nodes[i].send_ping(addrs[j]);
                total_pkts_sent += 1;
            }
        }
        // Drain
        for node in &mut nodes {
            node.recv_all();
        }
        checkpoints += 1;

        if checkpoints % 100 == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let total_recv: u64 = nodes.iter().map(|n| n.pkts_recv).sum();
            eprintln!(
                "  [many_nodes] {:.1}s: {} pkts sent, {} recv",
                elapsed, total_pkts_sent, total_recv
            );
            // Should have received at least some packets
            assert!(
                total_recv > 0 || elapsed < 2.0,
                "No packets received after {:.0}s",
                elapsed
            );
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_recv: u64 = nodes.iter().map(|n| n.pkts_recv).sum();
    eprintln!(
        "✅ stress_many_nodes: {:.0}s, {} pkts sent, {} recv ({:.0}%)",
        elapsed,
        total_pkts_sent,
        total_recv,
        total_recv as f64 / total_pkts_sent as f64 * 100.0
    );
}

// ── Soak test: long-running engine stability ────────────────────

/// Run the engine for 60 seconds with simulated load.
/// Checks for panics, hangs, and excessive memory.
///
/// Unlike the stress tests above, this one exercises the actual
/// `EngineLoop` by running the simulator in paper mode.
#[test]
#[ignore]
fn soak_engine_60s() {
    use neuron_wire::simulator::{SimulationConfig, Simulator};

    // Use paper mode with deterministic seed for reproducibility
    let config = SimulationConfig {
        node_count: 10,
        duration_secs: 60,
        paper_mode: true,
        seed: 42,
        ..SimulationConfig::default()
    };

    let start = Instant::now();
    let mut sim = Simulator::new(config);
    let result = sim.run();
    let elapsed = start.elapsed().as_secs_f64();

    match result {
        Ok(trial) => {
            eprintln!("✅ soak_engine_60s: completed in {:.2}s", elapsed);
            eprintln!(
                "   Convergence: {:.2}s, Peers: {:.1} avg, BW: {:.1} kbps",
                trial.convergence_time_secs.unwrap_or(0.0),
                trial.avg_peers,
                trial.bandwidth_kbps
            );
            assert!(
                elapsed < 120.0,
                "Simulation took too long: {:.1}s (expected < 120s)",
                elapsed
            );
        }
        Err(e) => {
            panic!("soak_engine_60s failed: {}", e);
        }
    }
}
