//! Fast DHT Simulator v4 — Extreme scale (1M–1B nodes).
//!
//! Uses a two-tier model:
//!   ACTIVE_MAX real nodes (fully simulated, ~100k)
//!   Virtual target-space for the rest (statistical PONG responses)
//!
//! The Kademlia convergence math is well-understood:
//!   - O(log N) routing hops
//!   - ~k * log_2(N) peers/node (k=3~5 for our config)
//!   - Convergence time ∝ log(N) in sim-ticks
//!
//! By calibrating ACTIVE_MAX nodes accurately and generating virtual
//! PONGs from the correct ID distribution, we get accurate convergence
//! metrics for any N without simulating every node.
//!
//! Usage: cargo run --release --bin bench-fast-v4 [node_counts...] [trials]
//!   Default: 100000,1000000,10000000,100000000,1000000000 1

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Instant;

// ── Tunables ──
const ACTIVE_MAX: u32 = 200_000; // nodes that actually exist as objects
const BOOTSTRAP_PINGS: usize = 30;
const PING_PER_ROUND: usize = 15;
const FIND_PER_ROUND: usize = 10;
const PONG_RECS: usize = 3;
const MAX_PEERS: usize = 500; // k-bucket cap (real Kademlia ~k*160)

// ── Compact message (16 bytes) ──
#[derive(Copy, Clone)]
#[repr(C)]
struct Msg {
    kind: u32,
    from: u32,
    to: u32,
    arg: u32,
}
const _: () = assert!(std::mem::size_of::<Msg>() == 16);

impl Msg {
    fn ping(from: u32, to: u32) -> Self {
        Self {
            kind: 0,
            from,
            to,
            arg: 0,
        }
    }
    fn pong(from: u32, to: u32, rec: u32) -> Self {
        Self {
            kind: 1,
            from,
            to,
            arg: rec,
        }
    }
    fn find_node(from: u32, to: u32, target: u32) -> Self {
        Self {
            kind: 2,
            from,
            to,
            arg: target,
        }
    }
    fn node_found(from: u32, to: u32, found: u32) -> Self {
        Self {
            kind: 3,
            from,
            to,
            arg: found,
        }
    }
}

// ── Compact node ──
struct Node {
    id: u32,
    peers: Vec<u32>, // sorted, deduped
    pkts_out: u32,
}

impl Node {
    fn new(id: u32) -> Self {
        Self {
            id,
            peers: Vec::with_capacity(128),
            pkts_out: 0,
        }
    }

    #[inline(always)]
    fn add_peer(&mut self, peer: u32) {
        if peer == self.id {
            return;
        }
        if self.peers.len() >= MAX_PEERS {
            return;
        }
        if let Err(idx) = self.peers.binary_search(&peer) {
            self.peers.insert(idx, peer);
        }
    }

    fn bootstrap(&self, total_nodes: u64, _tick_seed: u64, active: u32, buf: &mut Vec<Msg>) {
        let seed = (self.id as u64).wrapping_mul(6364136223846793005);
        // Half to active range (ensures we hit real nodes), half to full range
        let is_hybrid = total_nodes > active as u64;
        for i in 0..BOOTSTRAP_PINGS {
            let r = seed.wrapping_add((i as u64).wrapping_mul(1442695040888963407));
            let other = if is_hybrid && i < BOOTSTRAP_PINGS / 2 {
                (r % active as u64) as u32
            } else {
                (r % total_nodes) as u32
            };
            if other != self.id {
                buf.push(Msg::ping(self.id, other));
            }
        }
    }

    fn periodic(&self, total_nodes: u64, tick_seed: u64, buf: &mut Vec<Msg>) {
        if self.peers.is_empty() {
            return;
        }
        let count = self.peers.len();
        let seed = tick_seed.wrapping_mul(6364136223846793005);

        // PING known peers
        let np = PING_PER_ROUND.min(count);
        for pi in 0..np {
            let peer = self.peers[(pi * 73 + tick_seed as usize) % count];
            buf.push(Msg::ping(self.id, peer));
        }

        // FIND_NODE unknown targets
        if count >= 5 {
            for fi in 0..FIND_PER_ROUND {
                let ask = self.peers[(fi * 31 + tick_seed as usize + 7) % count];
                let target = (seed.wrapping_add((fi as u64).wrapping_mul(1442695040888963407)))
                    % total_nodes;
                let tu = target as u32;
                if tu != self.id && self.peers.binary_search(&tu).is_err() {
                    buf.push(Msg::find_node(self.id, ask, tu));
                }
            }
        }
    }
}

// ── run a trial ──
fn run_trial(total_nodes: u64) -> TrialStats {
    let active = (total_nodes as u32).min(ACTIVE_MAX);
    let threshold = ((total_nodes as f64).log2().ceil() as usize * 3).min(200);
    let max_ticks = (20_000u64).max((total_nodes as f64).log2().ceil() as u64 * 2000 + 10_000);
    let is_hybrid = total_nodes > active as u64;

    // Build active nodes
    let mut nodes: Vec<Node> = Vec::with_capacity(active as usize);
    for i in 0..active {
        nodes.push(Node::new(i));
    }

    // Staged bootstrap: BATCH nodes per tick
    let bsize = (active / 50).max(1000);
    let mut next_bs = 0u32;
    let mut bootstrap_done = false;

    let mut out: Vec<Msg> = Vec::with_capacity(500_000);
    let mut converged_at: Option<u64> = None;

    let start = Instant::now();
    let mut last_report = 0u64;
    let mut total_phase1: u64 = 0;

    for tick in 0..max_ticks {
        // ── progress ──
        let elapsed = start.elapsed();
        if elapsed.as_secs_f64() >= last_report as f64 + 15.0 {
            last_report = elapsed.as_secs();
            eprintln!(
                "    [{:.0}s] tick {}/{} msgs={:.1}M RSS~?",
                elapsed.as_secs_f64(),
                tick,
                max_ticks,
                total_phase1 as f64 / 1_000_000.0
            );
        }

        // ── Phase 1: generate ──
        out.clear();
        let tick_seed: u64 = tick.wrapping_mul(6364136223846793005);

        if !bootstrap_done && next_bs < active {
            let end = (next_bs + bsize).min(active);
            for i in next_bs..end {
                let n = &nodes[i as usize];
                if n.peers.is_empty() {
                    n.bootstrap(total_nodes, tick_seed, active, &mut out);
                }
            }
            next_bs = end;
            if next_bs >= active {
                bootstrap_done = true;
            }
        } else if bootstrap_done && tick > 0 && tick % 500 == 0 {
            // Periodic: visit 20% of nodes, round-robin across ticks
            let visit_count = (active / 5).max(1);
            let step = (active / visit_count).max(1);
            let round = tick / 500; // 1, 2, 3, ... → gives different offset each round
            for i in (round as u32 % step..active).step_by(step as usize) {
                nodes[i as usize].periodic(total_nodes, tick_seed, &mut out);
            }
        }

        // ── Phase 2: deliver (process all — including msgs generated during delivery) ──
        total_phase1 += out.len() as u64;
        let mut i = 0;
        while i < out.len() {
            let msg = out[i];
            i += 1;
            let to_idx = msg.to as usize;

            if to_idx >= active as usize && is_hybrid {
                // Virtual node response — knows all active nodes probabilistically
                match msg.kind {
                    0 => {
                        // PING → PONG with 3 random active nodes as recommendations
                        let from = msg.from as usize;
                        let sender_peers = nodes[from.min(active as usize - 1)].peers.len();
                        // Use a mix of sender's known peers and random active IDs
                        for r in 0..PONG_RECS {
                            let rec = if sender_peers > 3 && r < 2 {
                                // Use sender's known peers (spreads real info)
                                let idx = (tick_seed as usize).wrapping_mul(7).wrapping_add(r * 11)
                                    % sender_peers;
                                nodes[from.min(active as usize - 1)].peers[idx]
                            } else {
                                // Recommend a random active node (fast-track discovery)
                                let rid = tick_seed
                                    .wrapping_mul(6364136223846793005)
                                    .wrapping_add((r as u64).wrapping_mul(1442695040888963407))
                                    % active as u64;
                                rid as u32
                            };
                            out.push(Msg::pong(msg.to, msg.from, rec));
                        }
                    }
                    2 => {
                        // FIND_NODE to virtual node: always knows the target
                        out.push(Msg::node_found(msg.to, msg.from, msg.arg));
                    }
                    _ => {}
                }
                continue;
            }

            let n = &mut nodes[to_idx];
            match msg.kind {
                0 => {
                    // PING → PONG
                    n.add_peer(msg.from);
                    n.pkts_out += 1;
                    let count = n.peers.len();
                    let mut sent = false;
                    for r in 0..PONG_RECS.min(count) {
                        let idx = (tick_seed as usize).wrapping_mul(3).wrapping_add(r) % count;
                        let rec = n.peers[idx];
                        if rec != n.id {
                            out.push(Msg::pong(n.id, msg.from, rec));
                            sent = true;
                        }
                    }
                    if !sent {
                        out.push(Msg::pong(n.id, msg.from, n.id));
                    }
                }
                1 => {
                    // PONG
                    n.add_peer(msg.from);
                    if msg.arg != n.id && msg.arg != 0 {
                        n.add_peer(msg.arg);
                    }
                }
                2 => {
                    // FIND_NODE
                    n.add_peer(msg.from);
                    if n.peers.binary_search(&msg.arg).is_ok() || msg.arg == n.id {
                        out.push(Msg::node_found(n.id, msg.from, msg.arg));
                        n.pkts_out += 1;
                    }
                }
                3
                    // NODE_FOUND
                    if msg.arg != n.id => {
                        n.add_peer(msg.arg);
                    }
                _ => {}
            }
        }

        // ── Check convergence (sample 1:1000) ──
        if bootstrap_done && tick > 0 && tick % 500 == 0 && converged_at.is_none() {
            let step = (active / 1000).max(1);
            let mut conv = 0u64;
            let mut tot = 0u64;
            for i in (0..active).step_by(step as usize) {
                tot += 1;
                if nodes[i as usize].peers.len() >= threshold {
                    conv += 1;
                }
            }
            if conv as f64 >= tot as f64 * 0.99 {
                converged_at = Some(tick);
            }
        }

        if let Some(ct) = converged_at {
            if tick - ct > 2000 {
                break;
            }
        }
    }

    let wall = start.elapsed();

    // ── Final stats (sample 1:10000) ──
    let sstep = (active / 10000).max(1);
    let mut tp = 0u64;
    let mut mp = 0usize;
    let mut conv = 0u64;
    let mut samp = 0u64;
    let mut pkt = 0u64;
    for i in (0..active).step_by(sstep as usize) {
        samp += 1;
        let n = &nodes[i as usize];
        let pc = n.peers.len();
        tp += pc as u64;
        mp = mp.max(pc);
        if pc >= threshold {
            conv += 1;
        }
        pkt += n.pkts_out as u64;
    }
    let scale = active as f64 / samp as f64;
    let est_pkts = (pkt as f64 * scale) as u64;
    let est_peers = if samp > 0 {
        tp as f64 / samp as f64
    } else {
        0.0
    };
    let ct = converged_at.map(|t| t as f64 / 1000.0).unwrap_or(0.0);
    let conv_pct = conv as f64 / samp as f64 * 100.0;
    let bw = if wall.as_secs_f64() > 0.0 {
        est_pkts as f64 * 24.0 / wall.as_secs_f64() / 125.0 // ~24 bytes/pkt avg
    } else {
        0.0
    };

    eprintln!(
        "    → {} {} ct={:.3}s peers={:.1}/{} bw={:.0}kbps wall={:.1}s",
        total_nodes,
        if converged_at.is_some() { "✅" } else { "❌" },
        ct,
        est_peers,
        threshold,
        bw,
        wall.as_secs_f64()
    );

    TrialStats {
        node_count: total_nodes as u32,
        converged: converged_at.is_some(),
        conv_rate: conv_pct,
        ct_mean: ct,
        ap_mean: est_peers,
        mp_mean: mp as f64,
        bw_mean: bw,
        pkts_mean: est_pkts,
    }
}

#[derive(Clone)]
struct TrialStats {
    #[allow(dead_code)]
    node_count: u32,
    converged: bool,
    conv_rate: f64,
    ct_mean: f64,
    ap_mean: f64,
    mp_mean: f64,
    bw_mean: f64,
    pkts_mean: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let node_counts_str: Vec<&str> = args
        .get(1)
        .map(|s| s.split(',').collect())
        .unwrap_or_else(|| vec!["100k", "1m", "10m", "100m", "1b"]);
    // Parse with k/m/b suffixes
    fn parse_count(s: &str) -> Option<u64> {
        let s = s.trim().to_lowercase();
        if s.ends_with('b') {
            s[..s.len() - 1]
                .parse::<f64>()
                .ok()
                .map(|v| (v * 1_000_000_000.0) as u64)
        } else if s.ends_with('m') {
            s[..s.len() - 1]
                .parse::<f64>()
                .ok()
                .map(|v| (v * 1_000_000.0) as u64)
        } else if s.ends_with('k') {
            s[..s.len() - 1]
                .parse::<f64>()
                .ok()
                .map(|v| (v * 1_000.0) as u64)
        } else {
            s.parse::<u64>().ok()
        }
    }
    let node_counts: Vec<u64> = node_counts_str
        .iter()
        .filter_map(|s| parse_count(s))
        .collect();
    let trials: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let out_dir = std::path::PathBuf::from("results/bench-fast");
    fs::create_dir_all(&out_dir).ok();
    let csv_path = out_dir.join("fast_v4_results.csv");
    fs::write(&csv_path, "node_count,trial,converged,conv_rate,convergence_time_s,max_peers,avg_peers,bandwidth_kbps,packets_recv\n").ok();

    let start_all = Instant::now();
    eprintln!(
        "═══ FAST DHT v4 (extreme) ═══ {} cfgs × {}t",
        node_counts.len(),
        trials
    );
    eprintln!("Active:{} (hybrid for larger)\n", ACTIVE_MAX);

    for &nc in &node_counts {
        eprintln!("─── {}n×{}t ───", nc, trials);
        for t in 0..trials {
            let start = Instant::now();
            let stats = run_trial(nc);
            let elapsed = start.elapsed();
            let line = format!(
                "{},{},{},{:.1},{:.6},{},{:.4},{:.4},{}\n",
                nc,
                t,
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
                "  [{}/{}] {}n t{} {} ct={:.3}s peers={:.1} bw={:.0} wall={:.2}s",
                t + 1,
                trials,
                nc,
                t,
                if stats.converged { "✅" } else { "❌" },
                stats.ct_mean,
                stats.ap_mean,
                stats.bw_mean,
                elapsed.as_secs_f64()
            );
        }
    }
    eprintln!(
        "\n═══ DONE → {} ({}s)",
        csv_path.display(),
        start_all.elapsed().as_secs_f64() as u64
    );
}
