//! Distributed Hash Table — Latency-Weighted Hybrid Kademlia.
//!
//! Standard Kademlia uses XOR distance for bucket placement and time-based
//! eviction. Our hybrid uses XOR for bucket placement (guarantees global
//! reachability) and LATENCY for ranking/eviction within each bucket.
//!
//! ## Bootstrap Priority
//! 1. Peer cache file (from previous session)
//! 2. DNS seed resolution (`_dht.seeds.<domain>`)
//! 3. Hardcoded seed VPS addresses
//! 4. Passive listening (wait for gossip)

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::sync::mpsc::Sender;

use crate::engine_loop::{OutgoingPacket, Reliability, IngressEvent};
use crate::header;

// ─── Constants ─────────────────────────────────────────────────

const K: usize = 20;
const STALE_PING_S: u64 = 300;
const MAX_FAILURES: u32 = 3;

/// Seed nodes (hardcoded fallback when DNS unavailable).
/// REPLACE these with your own VPS addresses.
const SEED_NODES: &[&str] = &[
    // "203.0.113.1:9000",
    // "203.0.113.2:9000",
    // "198.51.100.1:9000",
];

// ─── NodeId (256-bit) ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn new(bytes: [u8; 32]) -> Self { NodeId(bytes) }

    /// XOR distance
    pub fn xor_distance(&self, other: &NodeId) -> [u8; 32] {
        let mut d = [0u8; 32];
        for i in 0..32 { d[i] = self.0[i] ^ other.0[i]; }
        d
    }

    /// Bucket index (0=furthest, 255=nearest), None if same node
    pub fn bucket_index(&self, other: &NodeId) -> Option<u8> {
        let dist = self.xor_distance(other);
        for (i, &byte) in dist.iter().enumerate() {
            if byte != 0 {
                let msb_within = 7 - (byte.leading_zeros() as u8);
                return Some((31 - i as u8) * 8 + msb_within);
            }
        }
        None
    }

    pub fn hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for &b in &self.0[..4] { s.push_str(&format!("{:02x}", b)); }
        s.push_str("…");
        for &b in &self.0[28..] { s.push_str(&format!("{:02x}", b)); }
        s
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hex())
    }
}

// ─── Node Type ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    General = 0, Language = 1, Reasoning = 2,
    Memory = 3,  Vision = 4,   Audio = 5,
    Consensus = 6, Gateway = 7,
}

impl NodeType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(NodeType::General),
            1 => Some(NodeType::Language),
            2 => Some(NodeType::Reasoning),
            3 => Some(NodeType::Memory),
            4 => Some(NodeType::Vision),
            5 => Some(NodeType::Audio),
            6 => Some(NodeType::Consensus),
            7 => Some(NodeType::Gateway),
            _ => None,
        }
    }
}

// ─── Node Entry ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub id: NodeId,
    pub addr: SocketAddr,
    pub latency_ms: f32,
    pub last_seen: Instant,
    pub node_type: NodeType,
    pub fail_count: u32,
}

impl NodeEntry {
    pub fn new(id: NodeId, addr: SocketAddr, node_type: NodeType) -> Self {
        NodeEntry { id, addr, latency_ms: 100.0, last_seen: Instant::now(), node_type, fail_count: 0 }
    }

    pub fn update_latency(&mut self, sample_ms: f32) {
        self.latency_ms = self.latency_ms * 0.7 + sample_ms * 0.3;
        self.last_seen = Instant::now();
        self.fail_count = 0;
    }

    pub fn record_failure(&mut self) { self.fail_count += 1; }
    pub fn is_dead(&self) -> bool { self.fail_count >= MAX_FAILURES }
}

// ─── K-Bucket ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KBucket {
    pub entries: Vec<NodeEntry>,
    pub max_size: usize,
}

impl KBucket {
    pub fn new() -> Self { KBucket { entries: Vec::with_capacity(K), max_size: K } }

    pub fn find(&self, id: &NodeId) -> Option<usize> {
        self.entries.iter().position(|e| e.id == *id)
    }

    /// Insert or update. Returns true if accepted.
    pub fn upsert(&mut self, entry: NodeEntry) -> bool {
        if let Some(idx) = self.find(&entry.id) {
            let e = &mut self.entries[idx];
            e.latency_ms = e.latency_ms * 0.7 + entry.latency_ms * 0.3;
            e.last_seen = Instant::now();
            e.fail_count = 0;
            self.sort_by_latency();
            return true;
        }
        if self.entries.len() < self.max_size {
            self.entries.push(entry);
            self.sort_by_latency();
            return true;
        }
        // Full: evict highest-latency if new node is faster
        let worst = self.entries.last().map(|e| e.latency_ms).unwrap_or(f32::MAX);
        if entry.latency_ms < worst {
            self.entries.pop();
            self.entries.push(entry);
            self.sort_by_latency();
            return true;
        }
        false
    }

    fn sort_by_latency(&mut self) {
        self.entries.sort_by(|a, b| {
            a.latency_ms.partial_cmp(&b.latency_ms).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Remove entry at index
    pub fn remove_at(&mut self, idx: usize) -> bool {
        if idx < self.entries.len() {
            self.entries.remove(idx);
            return true;
        }
        false
    }

    /// Remove entry by NodeId
    pub fn remove(&mut self, id: &NodeId) -> bool {
        if let Some(idx) = self.find(id) {
            self.entries.remove(idx);
            return true;
        }
        false
    }

    pub fn fastest(&self) -> Option<&NodeEntry> { self.entries.first() }
    pub fn len(&self) -> usize { self.entries.len() }
}

// ─── Routing Table ─────────────────────────────────────────────

pub struct RoutingTable {
    buckets: Vec<KBucket>,
    pub local_id: NodeId,
    pub local_addr: SocketAddr,
    pub total_nodes: usize,
    pub local_type: NodeType,
}

impl RoutingTable {
    pub fn new(local_id: NodeId, local_addr: SocketAddr, local_type: NodeType) -> Self {
        RoutingTable {
            buckets: (0..256).map(|_| KBucket::new()).collect(),
            local_id, local_addr, total_nodes: 0, local_type,
        }
    }

    fn bucket_idx(&self, target: &NodeId) -> Option<usize> {
        self.local_id.bucket_index(target).map(|i| i as usize)
    }

    /// Insert or update. Returns true if accepted.
    pub fn insert(&mut self, entry: NodeEntry) -> bool {
        let idx = match self.bucket_idx(&entry.id) {
            Some(i) => i,
            None => return false,
        };
        let ok = self.buckets[idx].upsert(entry);
        if ok { self.total_nodes = self.buckets.iter().map(|b| b.len()).sum(); }
        ok
    }

    pub fn remove(&mut self, id: &NodeId) -> bool {
        let idx = match self.bucket_idx(id) {
            Some(i) => i,
            None => return false,
        };
        let ok = self.buckets[idx].remove(id);
        if ok { self.total_nodes = self.buckets.iter().map(|b| b.len()).sum(); }
        ok
    }

    fn bucket_mut(&mut self, id: &NodeId) -> Option<(usize, &mut KBucket)> {
        self.bucket_idx(id).map(|i| {
            let bucket = &mut self.buckets[i];
            (i, bucket)
        })
    }

    pub fn record_latency(&mut self, id: &NodeId, sample: f32) {
        if let Some((_, bucket)) = self.bucket_mut(id) {
            if let Some(idx) = bucket.find(id) {
                bucket.entries[idx].update_latency(sample);
                bucket.sort_by_latency();
            }
        }
    }

    pub fn record_failure(&mut self, id: &NodeId) {
        if let Some((_, bucket)) = self.bucket_mut(id) {
            if let Some(idx) = bucket.find(id) {
                bucket.entries[idx].record_failure();
                if bucket.entries[idx].is_dead() {
                    bucket.remove_at(idx);
                    self.total_nodes = self.buckets.iter().map(|b| b.len()).sum();
                }
            }
        }
    }

    /// All nodes sorted by XOR distance to local_id (iterative lookup order)
    pub fn nearest_nodes(&self, target: &NodeId, count: usize) -> Vec<&NodeEntry> {
        let mut all: Vec<&NodeEntry> = self.buckets.iter().flat_map(|b| b.entries.iter()).collect();
        all.sort_by(|a, b| {
            let da = target.xor_distance(&a.id);
            let db = target.xor_distance(&b.id);
            for i in 0..32 {
                let c = da[i].cmp(&db[i]);
                if c != std::cmp::Ordering::Equal { return c; }
            }
            std::cmp::Ordering::Equal
        });
        all.truncate(count);
        all
    }

    /// Lowest-latency node in the nearest populated bucket to target
    pub fn closest_fast(&self, target: &NodeId) -> Option<&NodeEntry> {
        let idx = self.local_id.bucket_index(target)? as usize;
        for offset in 0..256 {
            for dir in [1usize, 1usize.wrapping_neg()] {
                let bi = if dir == 1 { idx.wrapping_add(offset) } else { idx.wrapping_sub(offset) };
                if bi < 256 {
                    if let Some(fastest) = self.buckets[bi].fastest() { return Some(fastest); }
                }
            }
        }
        None
    }

    pub fn all_nodes(&self) -> Vec<&NodeEntry> {
        self.buckets.iter().flat_map(|b| b.entries.iter()).collect()
    }

    pub fn node_count(&self) -> usize { self.total_nodes }
}

// ─── DHT Message Types ─────────────────────────────────────────

pub mod dht_msg_type {
    pub const PING: u8       = 7;
    pub const PONG: u8       = 8;
    pub const FIND_NODE: u8  = 9;
    pub const NODES: u8      = 10;
}

/// Field offsets for DHT FlatBuffer bodies
pub mod dht_fields {
    use crate::HEADER_SIZE;
    pub const SENDER_ID: usize     = HEADER_SIZE;
    pub const SENDER_ID_END: usize = HEADER_SIZE + 32;
    pub const NODE_TYPE: usize     = HEADER_SIZE + 42;
    pub const LATENCY_MS: usize    = HEADER_SIZE + 44;
    pub const PING_SEQ: usize      = HEADER_SIZE + 48;
    pub const PING_BODY_SIZE: usize = 52;
    pub const PONG_BODY_SIZE: usize = 52;
    pub const TARGET_ID: usize     = HEADER_SIZE;
    pub const FIND_BODY_SIZE: usize = 32;
}

// ─── Serialization ─────────────────────────────────────────────

fn encode_addr(addr: &SocketAddr, buf: &mut Vec<u8>) {
    match addr {
        SocketAddr::V4(v4) => {
            buf.push(4u8);
            buf.extend_from_slice(&v4.ip().octets());
            buf.extend_from_slice(&v4.port().to_be_bytes());
        }
        SocketAddr::V6(v6) => {
            buf.push(6u8);
            buf.extend_from_slice(&v6.ip().octets());
            buf.extend_from_slice(&v6.port().to_be_bytes());
        }
    }
}

fn serialized_addr_size(addr: &SocketAddr) -> usize {
    match addr {
        SocketAddr::V4(_) => 1 + 4 + 2,   // family + ip + port
        SocketAddr::V6(_) => 1 + 16 + 2,
    }
}

fn decode_addr(data: &[u8], offset: &mut usize) -> Option<SocketAddr> {
    if *offset + 1 > data.len() { return None; }
    let af = data[*offset];
    *offset += 1;
    match af {
        4 => {
            if *offset + 6 > data.len() { return None; }
            let ip = std::net::Ipv4Addr::new(data[*offset], data[*offset+1], data[*offset+2], data[*offset+3]);
            *offset += 4;
            let port = u16::from_be_bytes([data[*offset], data[*offset+1]]);
            *offset += 2;
            Some(SocketAddr::V4(std::net::SocketAddrV4::new(ip, port)))
        }
        6 => {
            if *offset + 18 > data.len() { return None; }
            let mut octs = [0u8; 16];
            octs.copy_from_slice(&data[*offset..*offset+16]);
            *offset += 16;
            let port = u16::from_be_bytes([data[*offset], data[*offset+1]]);
            *offset += 2;
            Some(SocketAddr::V6(std::net::SocketAddrV6::new(octs.into(), port, 0, 0)))
        }
        _ => None,
    }
}

/// Serialize a NodeEntry into the buffer (for NODES responses)
pub fn serialize_node(entry: &NodeEntry, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&entry.id.0);
    encode_addr(&entry.addr, buf);
    buf.push(entry.node_type as u8);
    buf.extend_from_slice(&(entry.latency_ms as u32).to_le_bytes());
}

/// Compute the serialized size of this entry
pub fn serialized_node_size(entry: &NodeEntry) -> usize {
    32 + serialized_addr_size(&entry.addr) + 1 + 4
}

/// Deserialize a NodeEntry from `data` starting at `offset`
pub fn deserialize_node(data: &[u8], offset: &mut usize) -> Option<NodeEntry> {
    if *offset + 32 > data.len() { return None; }
    let mut id = [0u8; 32];
    id.copy_from_slice(&data[*offset..*offset+32]);
    *offset += 32;

    let addr = decode_addr(data, offset)?;

    if *offset + 1 > data.len() { return None; }
    let nt = NodeType::from_u8(data[*offset]).unwrap_or(NodeType::General);
    *offset += 1;

    if *offset + 4 > data.len() { return None; }
    let _lat = u32::from_le_bytes([data[*offset], data[*offset+1], data[*offset+2], data[*offset+3]]) as f32;
    *offset += 4;

    Some(NodeEntry::new(NodeId(id), addr, nt))
}

// ─── DHT Handler ───────────────────────────────────────────────

pub struct DhtHandler {
    pub routing_table: RoutingTable,
    outbound_tx: Sender<OutgoingPacket>,
    pub pending_pings: HashMap<u32, Instant>,
    next_ping_seq: u32,
    cache_path: Option<String>,
    seed_domain: String,
    bootstrapped: bool,
}

impl DhtHandler {
    pub fn new(
        local_id: NodeId, local_addr: SocketAddr, local_type: NodeType,
        outbound_tx: Sender<OutgoingPacket>,
        cache_path: Option<String>, seed_domain: String,
    ) -> Self {
        DhtHandler {
            routing_table: RoutingTable::new(local_id, local_addr, local_type),
            outbound_tx,
            pending_pings: HashMap::new(),
            next_ping_seq: 1,
            cache_path, seed_domain, bootstrapped: false,
        }
    }

    // ─── Bootstrap ──────────────────────────────────────────

    pub fn bootstrap(&mut self) {
        if self.bootstrapped { return; }
        self.bootstrapped = true;

        // 1. Peer cache
        if let Some(ref path) = self.cache_path {
            if let Ok(nodes) = load_peers(path) {
                if !nodes.is_empty() {
                    eprintln!("[DHT] Loaded {} cached peers", nodes.len());
                    for (addr, _id) in &nodes { self.ping_node(*addr); }
                    return;
                }
            }
        }

        // 2. DNS seeds (resolve hostname)
        let seeds = resolve_dns_seeds(&self.seed_domain);
        if !seeds.is_empty() {
            eprintln!("[DHT] DNS seeds resolved: {}", seeds.len());
            for addr in seeds { self.ping_node(addr); }
            return;
        }

        // 3. Hardcoded seeds
        for s in SEED_NODES {
            if let Ok(addr) = s.parse::<SocketAddr>() {
                eprintln!("[DHT] Hardcoded seed: {}", addr);
                self.ping_node(addr);
            }
        }

        eprintln!("[DHT] No seeds — listening passively");
    }

    // ─── Ingress ────────────────────────────────────────────

    pub fn handle_event(&mut self, event: &IngressEvent) {
        let payload = &event.nwp_payload;
        if payload.len() < crate::HEADER_SIZE + 1 { return; }
        let msg_type = payload[5]; // offset in NWP header
        match msg_type {
            dht_msg_type::PING      => self.handle_ping(event),
            dht_msg_type::PONG      => self.handle_pong(event, payload),
            dht_msg_type::FIND_NODE => self.handle_find_node(event, payload),
            dht_msg_type::NODES     => self.handle_nodes(payload),
            _ => {}
        }
    }

    fn handle_ping(&mut self, event: &IngressEvent) {
        let payload = &event.nwp_payload;
        if payload.len() < dht_fields::PING_BODY_SIZE { return; }
        let mut sid = [0u8; 32];
        sid.copy_from_slice(&payload[dht_fields::SENDER_ID..dht_fields::SENDER_ID_END]);
        let sender = NodeId(sid);
        let node_type = NodeType::from_u8(payload[dht_fields::NODE_TYPE]).unwrap_or(NodeType::General);

        let mut entry = NodeEntry::new(sender, event.src, node_type);
        entry.update_latency(100.0);
        self.routing_table.insert(entry);

        let seq = u32::from_le_bytes([
            payload[dht_fields::PING_SEQ], payload[dht_fields::PING_SEQ+1],
            payload[dht_fields::PING_SEQ+2], payload[dht_fields::PING_SEQ+3],
        ]);
        self.send_pong(event.src, seq);
    }

    fn handle_pong(&mut self, _event: &IngressEvent, payload: &[u8]) {
        if payload.len() < dht_fields::PONG_BODY_SIZE { return; }
        let mut sid = [0u8; 32];
        sid.copy_from_slice(&payload[dht_fields::SENDER_ID..dht_fields::SENDER_ID_END]);
        let sender = NodeId(sid);

        let seq = u32::from_le_bytes([
            payload[dht_fields::PING_SEQ], payload[dht_fields::PING_SEQ+1],
            payload[dht_fields::PING_SEQ+2], payload[dht_fields::PING_SEQ+3],
        ]);

        let rtt = self.pending_pings.remove(&seq)
            .map(|t| t.elapsed().as_millis() as f32)
            .unwrap_or(100.0);

        self.routing_table.record_latency(&sender, rtt);
    }

    fn handle_find_node(&mut self, event: &IngressEvent, payload: &[u8]) {
        if payload.len() < dht_fields::FIND_BODY_SIZE { return; }
        let mut tid = [0u8; 32];
        tid.copy_from_slice(&payload[dht_fields::TARGET_ID..dht_fields::TARGET_ID+32]);
        let target = NodeId(tid);

        // Clone results to avoid borrow conflict
        let nearest: Vec<(NodeId, SocketAddr, NodeType, f32)> = {
            self.routing_table.nearest_nodes(&target, K).iter().map(|e| {
                (e.id, e.addr, e.node_type, e.latency_ms)
            }).collect()
        };
        self.send_nodes(event.src, target, &nearest);
    }

    fn handle_nodes(&mut self, payload: &[u8]) {
        if payload.len() <= crate::HEADER_SIZE { return; }
        let mut offset = crate::HEADER_SIZE;
        let mut added = 0;
        while offset < payload.len() {
            if let Some(mut node) = deserialize_node(payload, &mut offset) {
                if node.id != self.routing_table.local_id {
                    node.latency_ms = 100.0;
                    if self.routing_table.insert(node) { added += 1; }
                }
            } else {
                break; // corrupt or end
            }
        }
        if added > 0 {
            eprintln!("[DHT] +{} nodes from NODES response", added);
        }
    }

    // ─── Outbound ──────────────────────────────────────────

    pub fn ping_node(&mut self, dst: SocketAddr) {
        let seq = self.next_ping_seq;
        self.next_ping_seq = self.next_ping_seq.wrapping_add(1);
        self.pending_pings.insert(seq, Instant::now());

        let mut body = Vec::with_capacity(60);
        body.extend_from_slice(&self.routing_table.local_id.0);
        encode_addr(&self.routing_table.local_addr, &mut body);
        body.push(self.routing_table.local_type as u8);
        body.extend_from_slice(&0u32.to_le_bytes()); // latency placeholder
        body.extend_from_slice(&seq.to_le_bytes());

        let frame = header::build_frame(dht_msg_type::PING, body, 0);
        let _ = self.outbound_tx.send(OutgoingPacket {
            payload: frame, dst, mode: Reliability::BestEffort,
        });
    }

    fn send_pong(&mut self, dst: SocketAddr, ping_seq: u32) {
        let mut body = Vec::with_capacity(60);
        body.extend_from_slice(&self.routing_table.local_id.0);
        encode_addr(&self.routing_table.local_addr, &mut body);
        body.push(self.routing_table.local_type as u8);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&ping_seq.to_le_bytes());

        let frame = header::build_frame(dht_msg_type::PONG, body, 0);
        let _ = self.outbound_tx.send(OutgoingPacket {
            payload: frame, dst, mode: Reliability::BestEffort,
        });
    }

    pub fn find_node(&mut self, target: NodeId, dst: SocketAddr) {
        let mut body = Vec::with_capacity(32);
        body.extend_from_slice(&target.0);
        let frame = header::build_frame(dht_msg_type::FIND_NODE, body, 0);
        let _ = self.outbound_tx.send(OutgoingPacket {
            payload: frame, dst, mode: Reliability::Data,
        });
    }

    fn send_nodes(&mut self, dst: SocketAddr, target: NodeId, nodes: &[(NodeId, SocketAddr, NodeType, f32)]) {
        let mut body = Vec::new();
        body.extend_from_slice(&target.0);
        for (id, addr, nt, lat) in nodes {
            let e = NodeEntry { id: *id, addr: *addr, latency_ms: *lat, last_seen: Instant::now(), node_type: *nt, fail_count: 0 };
            serialize_node(&e, &mut body);
        }
        let frame = header::build_frame(dht_msg_type::NODES, body, 0);
        let _ = self.outbound_tx.send(OutgoingPacket {
            payload: frame, dst, mode: Reliability::Data,
        });
    }

    // ─── Maintenance ──────────────────────────────────────

    pub fn periodic_maintenance(&mut self) {
        let now = Instant::now();
        let cutoff = Duration::from_secs(STALE_PING_S);

        // Collect stale addrs first, then ping (avoid borrow conflict)
        let stale: Vec<SocketAddr> = self.routing_table.all_nodes().iter()
            .filter(|e| now.duration_since(e.last_seen) > cutoff)
            .map(|e| e.addr)
            .collect();

        for addr in stale {
            self.ping_node(addr);
        }

        if let Some(ref path) = self.cache_path {
            save_peers(&self.routing_table, path).ok();
        }

        eprintln!("[DHT] {} nodes, {} pending pings",
            self.routing_table.node_count(), self.pending_pings.len());
    }
}

// ─── Free Functions ────────────────────────────────────────────

/// Resolve DNS hostname to IP:9000 addresses
fn resolve_dns_seeds(domain: &str) -> Vec<SocketAddr> {
    use std::net::ToSocketAddrs;
    let host_port = format!("{}:9000", domain);
    match host_port.to_socket_addrs() {
        Ok(iter) => iter.take(10).collect(),
        Err(_) => vec![],
    }
}

fn load_peers(path: &str) -> std::io::Result<Vec<(SocketAddr, NodeId)>> {
    let data = std::fs::read(path)?;
    let mut peers = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        if let Some(node) = deserialize_node(&data, &mut offset) {
            peers.push((node.addr, node.id));
        } else {
            break;
        }
    }
    Ok(peers)
}

fn save_peers(table: &RoutingTable, path: &str) -> std::io::Result<()> {
    let mut buf = Vec::new();
    for e in table.all_nodes() {
        serialize_node(e, &mut buf);
    }
    std::fs::write(path, &buf)
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(b: u8) -> NodeId { let mut a=[0u8;32]; a[31]=b; NodeId(a) }

    #[test]
    fn test_xor_distance() {
        let d = nid(0x00).xor_distance(&nid(0xFF));
        assert_eq!(d[31], 0xFF);
    }

    #[test]
    fn test_bucket_index() {
        // XOR = 0x01 (only LSB differs) → MSB at position 0 → bucket 0
        assert_eq!(nid(0x00).bucket_index(&nid(0x01)), Some(0));
        // XOR = 0x80 → MSB at position 7 → bucket 7
        assert_eq!(nid(0x00).bucket_index(&nid(0x80)), Some(7));
        // Same node
        assert_eq!(nid(0x00).bucket_index(&nid(0x00)), None);
    }

    #[test]
    fn test_routing_insert() {
        let local = nid(0x00);
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let mut rt = RoutingTable::new(local, addr, NodeType::General);
        for i in 0..K+1 {
            let mut id = [0u8;32]; id[31] = i as u8;
            rt.insert(NodeEntry::new(NodeId(id), addr, NodeType::General));
        }
        assert!(rt.node_count() <= K);
    }

    #[test]
    fn test_latency_eviction() {
        let local = nid(0x00);
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let mut rt = RoutingTable::new(local, addr, NodeType::General);
        // All nodes differ at bit 7 (bucket 7): IDs 0x80..0x93
        for i in 0..K {
            let mut id = [0u8;32]; id[31] = 0x80 + i as u8;
            let mut e = NodeEntry::new(NodeId(id), addr, NodeType::General);
            e.latency_ms = 200.0 + (i as f32) * 10.0;
            rt.insert(e);
        }
        assert_eq!(rt.node_count(), K);
        // Low-latency node in same bucket (also differs at bit 7)
        let mut fast = NodeEntry::new(nid(0xFF), addr, NodeType::General);
        fast.latency_ms = 5.0;
        assert!(rt.insert(fast), "low-latency should evict highest-latency");
        assert_eq!(rt.node_count(), K, "should stay at K after eviction");
    }

    #[test]
    fn test_node_type_roundtrip() {
        assert_eq!(NodeType::from_u8(0), Some(NodeType::General));
        assert_eq!(NodeType::from_u8(7), Some(NodeType::Gateway));
        assert_eq!(NodeType::from_u8(99), None);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        let e = NodeEntry::new(nid(0x42), addr, NodeType::Reasoning);
        let mut buf = Vec::new();
        serialize_node(&e, &mut buf);
        let mut off = 0;
        let d = deserialize_node(&buf, &mut off).unwrap();
        assert_eq!(d.id, e.id);
        assert_eq!(d.addr, e.addr);
        assert_eq!(d.node_type, e.node_type);
    }

    #[test]
    fn test_nearest_nodes() {
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let mut rt = RoutingTable::new(nid(0x00), addr, NodeType::General);
        for i in 0..10 {
            let mut id = [0u8;32]; id[31] = i;
            rt.insert(NodeEntry::new(NodeId(id), addr, NodeType::General));
        }
        assert_eq!(rt.nearest_nodes(&nid(0x05), 3).len(), 3);
    }
}
