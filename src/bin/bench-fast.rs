//! Fast In-Process DHT Simulator v3 — Bulk-optimized for 100k nodes.
//!
//! No UDP sockets, no OS threads per node.
//! Uses gossip PEX + Kademlia FIND_NODE, without periodic peer heartbeat flood.
//!
//! Usage: cargo run --release --bin bench-fast [node_counts...] [trials] [max_peers]
//!   Default: 100,1000,10000,50000,100000 3
//!   max_peers: 0 = unbounded full-registry mode (default); N>0 = bounded-production
//!   routing mode (FIFO eviction at cap, mirroring the engine's max_peers bound).

use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Instant;

const MAX_TICKS: u64 = 120_000; // sim ticks per trial (120s simulated, ~0.1-10s real)

// ── Node ──
struct Node {
    id: u64,
    peers: HashSet<u64>,
    peer_order: VecDeque<u64>, // FIFO insertion order for bounded eviction
    max_peers: usize,          // 0 = unbounded (full-registry research mode)
    pkts_out: u64,
    pkts_in: u64,
    bytes_out: u64,
    bytes_in: u64,
    converged: bool,
}

impl Node {
    fn new(id: u64, max_peers: usize) -> Self {
        Self {
            id,
            peers: HashSet::new(),
            peer_order: VecDeque::new(),
            max_peers,
            pkts_out: 0,
            pkts_in: 0,
            bytes_out: 0,
            bytes_in: 0,
            converged: false,
        }
    }

    /// Insert a peer, honoring the routing-table cap (FIFO eviction, mirroring the
    /// engine's `max_peers` bound). Unbounded mode (0) keeps full-registry behavior.
    fn insert_peer(&mut self, p: u64) {
        if p == self.id || self.peers.contains(&p) {
            return;
        }
        if self.max_peers > 0 && self.peers.len() >= self.max_peers {
            // Evict oldest-known peer to make room (Kademlia-style bounded table).
            if let Some(oldest) = self.peer_order.pop_front() {
                self.peers.remove(&oldest);
            }
        }
        self.peers.insert(p);
        self.peer_order.push_back(p);
    }

    /// Generate messages for this tick.
    fn tick(&mut self, tick: u64, all_count: u32, msgq: &mut Vec<Message>) {
        // Bootstrap: ping 30 random nodes (enough for up to 100k at threshold log2(N)*3)
        if self.peers.is_empty() {
            let seed = self.id.wrapping_mul(6364136223846793005);
            for i in 0..30 {
                let rnd = seed.wrapping_add((i as u64).wrapping_mul(1442695040888963407));
                let other = rnd % all_count as u64;
                if other != self.id {
                    msgq.push(Message::Ping(self.id, other));
                    self.pkts_out += 1;
                    self.bytes_out += 24;
                }
            }
            return;
        }

        // Periodic: PING 15 peers + FIND_NODE 10 unknowns
        if tick > 0 && tick.is_multiple_of(1000) {
            let v: Vec<u64> = self.peers.iter().copied().collect();
            let count = v.len();

            // PING 15 random peers (PONG gives 3 recs each)
            for pi in 0..15.min(count) {
                let peer = v[(pi * 73 + tick as usize) % count];
                msgq.push(Message::Ping(self.id, peer));
                self.pkts_out += 1;
                self.bytes_out += 24;
            }

            // FIND_NODE for 10 unknown nodes
            if count >= 5 {
                for _ in 0..10 {
                    let ask_peer = v[(tick as usize + 7) % count];
                    let mut target = (self.id.wrapping_add(tick).wrapping_mul(6364136223846793005))
                        % all_count as u64;
                    for _ in 0..200 {
                        if target != self.id && !self.peers.contains(&target) {
                            msgq.push(Message::FindNode(self.id, ask_peer, target));
                            self.pkts_out += 1;
                            self.bytes_out += 32;
                            break;
                        }
                        target = (target + 1) % all_count as u64;
                    }
                }
            }
        }
    }

    fn recv(&mut self, msg: &Message, msgq: &mut Vec<Message>, tick: u64, _all_count: u32) {
        self.pkts_in += 1;
        self.bytes_in += msg.wire_size() as u64;
        match *msg {
            Message::Ping(from, to) if to == self.id => {
                self.insert_peer(from);
                // PONG back with 3 random peer recommendations (offset by tick for variety)
                let v: Vec<u64> = self.peers.iter().copied().collect();
                let len = v.len();
                for r in 0..3 {
                    let idx = ((tick as usize).wrapping_mul(3).wrapping_add(r)) % len.max(1);
                    let rec = v[idx];
                    if rec != self.id {
                        msgq.push(Message::Pong(self.id, from, rec));
                        self.pkts_out += 1;
                        self.bytes_out += 28;
                    }
                }
            }
            Message::Pong(from, to, rec) if to == self.id => {
                self.insert_peer(from);
                if rec != self.id && rec != 0 {
                    self.insert_peer(rec);
                }
            }
            Message::FindNode(from, to, target) if to == self.id => {
                self.insert_peer(from);
                if self.peers.contains(&target) || target == self.id {
                    msgq.push(Message::NodeFound(from, target));
                    self.pkts_out += 1;
                    self.bytes_out += 28;
                } else if let Some(&closer) = self.peers.iter().next() {
                    msgq.push(Message::FindNode(from, closer, target));
                    self.pkts_out += 1;
                    self.bytes_out += 32;
                }
            }
            Message::NodeFound(to, found) if to == self.id && found != self.id => {
                self.insert_peer(found);
            }
            _ => {}
        }
    }
}

// ── Message ──
#[derive(Copy, Clone)]
enum Message {
    Ping(u64, u64),          // from, to
    Pong(u64, u64, u64),     // from, to, rec_peer
    FindNode(u64, u64, u64), // from, to, target
    NodeFound(u64, u64),     // to, found
}

impl Message {
    fn wire_size(&self) -> usize {
        match self {
            Message::Ping(..) => 24,
            Message::Pong(..) => 28,
            Message::FindNode(..) => 32,
            Message::NodeFound(..) => 28,
        }
    }

    fn to(&self) -> u64 {
        match *self {
            Message::Ping(_, to) => to,
            Message::Pong(_, to, _) => to,
            Message::FindNode(_, to, _) => to,
            Message::NodeFound(to, _) => to,
        }
    }
}

// ── Run one trial ──
fn run_trial(num_nodes: u32, max_peers: usize) -> TrialStats {
    let mut nodes: Vec<Node> = (0..num_nodes)
        .map(|i| Node::new(i as u64, max_peers))
        .collect();
    let min_peers = ((num_nodes as f64).log2().ceil() * 3.0) as usize;
    let mut msgq: Vec<Message> = Vec::with_capacity(1_000_000);
    let mut converged_at: Option<u64> = None;
    let mut conv_count: u32 = 0;

    for tick in 0..MAX_TICKS {
        // Early exit: 80%+ converged, stay 5000 ticks to stabilize
        if converged_at.is_some() && tick - converged_at.unwrap() > 5000 {
            break;
        }

        // Phase 1: produce messages
        for n in &mut nodes {
            n.tick(tick, num_nodes, &mut msgq);
        }

        // Phase 2: deliver messages (in-place drain)
        let len = msgq.len();
        for i in 0..len {
            let msg = unsafe { ptr_read(&msgq, i) };
            let to = msg.to() as usize;
            if to < nodes.len() {
                nodes[to].recv(&msg, &mut msgq, tick, num_nodes);
            }
        }
        msgq.clear();

        // Check convergence every 500 ticks
        if converged_at.is_none() && tick % 500 == 0 {
            conv_count = 0;
            for n in &mut nodes {
                if !n.converged && n.peers.len() >= min_peers {
                    n.converged = true;
                    conv_count += 1;
                } else if n.converged {
                    conv_count += 1;
                }
            }
            if conv_count as f64 >= num_nodes as f64 * 0.8 {
                converged_at = Some(tick);
            }
        }
    }

    // Stats
    let total_pkts: u64 = nodes.iter().map(|n| n.pkts_out).sum();
    let total_bytes: u64 = nodes.iter().map(|n| n.bytes_out).sum();
    let avg_peers: f64 =
        nodes.iter().map(|n| n.peers.len() as f64).sum::<f64>() / nodes.len() as f64;
    let max_peers = nodes.iter().map(|n| n.peers.len()).max().unwrap_or(0);
    let converged = converged_at.is_some();
    let ct = converged_at.map(|t| t as f64 / 1000.0).unwrap_or(0.0);
    let bw = if MAX_TICKS > 0 {
        total_bytes as f64 / (MAX_TICKS as f64 / 1000.0) / 125.0
    } else {
        0.0
    };

    TrialStats {
        node_count: num_nodes,
        trials: 1,
        converged,
        conv_rate: conv_count as f64 / num_nodes as f64 * 100.0,
        ct_mean: ct,
        ct_std: 0.0,
        ct_min: ct,
        ct_max: ct,
        bw_mean: bw,
        bw_min: bw,
        bw_max: bw,
        ap_mean: avg_peers,
        mp_mean: max_peers as f64,
        pkts_mean: total_pkts,
        peer_cap: nodes.first().map(|n| n.max_peers).unwrap_or(0),
    }
}

// Safe transmute-style read without consuming
unsafe fn ptr_read<T>(v: &[T], i: usize) -> T
where
    T: Copy,
{
    std::ptr::read(v.as_ptr().add(i))
}

/// Trial statistics for CSV output.
#[derive(Clone)]
#[allow(dead_code)]
struct TrialStats {
    node_count: u32,
    trials: u32,
    converged: bool,
    conv_rate: f64,
    ct_mean: f64,
    ct_std: f64,
    ct_min: f64,
    ct_max: f64,
    bw_mean: f64,
    bw_min: f64,
    bw_max: f64,
    ap_mean: f64,
    mp_mean: f64,
    pkts_mean: u64,
    peer_cap: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let node_counts: Vec<u32> = args
        .get(1)
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![100, 1000, 10000, 50000, 100000]);
    let trials: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    // max_peers: 0 = unbounded full-registry mode (default); N>0 = bounded routing.
    let max_peers: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

    let out_dir = std::path::PathBuf::from("results/bench-fast");
    fs::create_dir_all(&out_dir).ok();
    let csv_path = out_dir.join("fast_scaling_results.csv");
    fs::write(
        &csv_path,
        "node_count,trial,peer_cap,converged,conv_rate,convergence_time_s,max_peers,avg_peers,bandwidth_kbps,packets_recv\n",
    )
    .ok();

    let total = node_counts.len() as u32 * trials;
    let start_all = Instant::now();

    eprintln!(
        "═══════ FAST DHT v3 ═══════ {} cfgs × {} trials = {} runs (scale-optimized, peer_cap={})",
        node_counts.len(),
        trials,
        total,
        max_peers
    );
    eprintln!("No periodic peer heartbeats; gossip PEX + FIND_NODE every 500 ticks");
    eprintln!("Max {} sim-ticks per trial\n", MAX_TICKS);

    for &nc in &node_counts {
        eprintln!("─── {}n × {}t ───", nc, trials);
        for t in 0..trials {
            let start = Instant::now();
            let stats = run_trial(nc, max_peers);
            let elapsed = start.elapsed();

            let line = format!(
                "{},{},{},{},{:.1},{:.4},{},{:.4},{:.4},{}\n",
                nc,
                t,
                stats.peer_cap,
                stats.converged,
                stats.conv_rate,
                stats.ct_mean,
                stats.mp_mean as u64,
                stats.ap_mean,
                stats.bw_mean,
                stats.pkts_mean
            );
            let mut f = OpenOptions::new()
                .append(true)
                .create(true)
                .open(&csv_path)
                .unwrap();
            f.write_all(line.as_bytes()).ok();

            eprintln!(
                "  [{:>2}/{}] {:>6}n t{} {} ct={:.2}s peers={:.1}/{} bw={:.0} pkt={} wall={:.2}s",
                t + 1,
                trials,
                nc,
                t,
                if stats.converged { "✅" } else { "❌" },
                stats.ct_mean,
                stats.ap_mean,
                ((nc as f64).log2().ceil() * 3.0) as u64,
                stats.bw_mean,
                stats.pkts_mean,
                elapsed.as_secs_f64()
            );
        }
    }

    let total_elapsed = start_all.elapsed();
    eprintln!(
        "\n═══════ DONE → {} ({}s)",
        csv_path.display(),
        total_elapsed.as_secs_f64() as u64
    );
}
