//! nwp-demo — WebAssembly browser demo of P2P neural computation.
//!
//! Open one tab. Open another. They discover each other. Learn together.
//! No server. No install. Pure WASM in the browser.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;

// ─── Global State ──────────────────────────────────────────────────────────

/// The singleton demo node, locked behind a Mutex for safe WASM access.
static NODE: Mutex<Option<DemoNode>> = Mutex::new(None);

/// Messages queued for BroadcastChannel delivery.
/// JS reads these after each tick and sends them.
static OUTGOING: Mutex<Vec<OutgoingMessage>> = Mutex::new(Vec::new());

/// Events for visual feedback (packet flashes, peer changes).
static RECENT_EVENTS: Mutex<Vec<DemoEvent>> = Mutex::new(Vec::new());

// ─── Constants ─────────────────────────────────────────────────────────────

const NEURON_COUNT: usize = 6;
const K_BUCKET_SIZE: usize = 4;
const HEARTBEAT_INTERVAL_TICKS: u64 = 30;  // every ~500ms at 60fps
const GRADIENT_EXCHANGE_INTERVAL: u64 = 60; // every ~1s
const PEER_TIMEOUT_TICKS: u64 = 180;       // 3s without heartbeat = dead
const HEBBIAN_ETA: f32 = 0.05;
const HEBBIAN_LAMBDA: f32 = 0.01;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
struct Neuron {
    id: u32,
    activation: f32,
    bias: f32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Clone, Serialize, Deserialize)]
struct Synapse {
    from: u32,
    to: u32,
    weight: f32,
}

#[derive(Clone, Serialize)]
struct Peer {
    id: String,
    latency_ticks: u32,
    last_seen_tick: u64,
    alive: bool,
    activations: Vec<f32>,  // last received activations
    x: f32,
    y: f32,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
enum DemoEvent {
    PacketSent { to: String },
    PacketRecv { from: String },
    PeerDiscovered { id: String },
    PeerLost { id: String },
    NeuronFired { id: u32 },
}

#[derive(Clone, Serialize)]
struct OutgoingMessage {
    channel: String,  // "broadcast" for all, or peer ID for direct
    msg_type: String,
    body: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct GradientMessage {
    from: String,
    tick: u64,
    activations: Vec<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
struct HeartbeatMessage {
    from: String,
    tick: u64,
    neuron_count: usize,
    avg_activation: f32,
}

/// The core demo node.
struct DemoNode {
    id: String,
    tick: u64,

    // Neural network
    neurons: Vec<Neuron>,
    synapses: Vec<Synapse>,

    // Peer discovery
    peers: HashMap<String, Peer>,
    peer_id_counter: u32,

    // Simple PRNG (LCG) — no external rand crate needed
    rng: u32,

    // Stats
    packets_sent: u64,
    packets_recv: u64,
    ticks_per_sec: f64,
    last_tick_time: f64,

    // Packets in flight (visual animation)
    packets_in_flight: Vec<PacketAnim>,
}

#[derive(Clone, Serialize)]
struct PacketAnim {
    id: u32,
    peer_id: String,
    direction: String, // "out" or "in"
    progress: f32,     // 0.0 -> 1.0
}

// ─── WASM Exports ───────────────────────────────────────────────────────────

/// Initialise the demo node. Called once from JS on page load.
/// `seed_floats` is a JSON array of 20 random f32 values from JS's Math.random().
#[wasm_bindgen]
pub fn init(tab_id: &str, seed_floats: &str) {
    let seeds: Vec<f32> = serde_json::from_str(seed_floats).unwrap_or_else(|_| vec![0.5; 20]);
    let mut rng = (seeds[0] * 65536.0) as u32;
    if rng == 0 { rng = 1; }

    let node = DemoNode::new(tab_id, &mut rng, seeds);
    *NODE.lock().unwrap() = Some(node);
}

/// Run one simulation tick. Returns JSON state for JS rendering.
#[wasm_bindgen]
pub fn tick(dt_ms: f64) -> String {
    let mut guard = NODE.lock().unwrap();
    let node = guard.as_mut().expect("DemoNode not initialised — call init() first");
    let state = node.tick(dt_ms);
    serde_json::to_string(&state).unwrap_or_else(|_| "null".into())
}

/// Handle an incoming message from another tab (via BroadcastChannel).
#[wasm_bindgen]
pub fn on_message(msg: &str) {
    let mut guard = NODE.lock().unwrap();
    if let Some(ref mut node) = *guard {
        node.handle_message(msg);
    }
}

/// Remove a peer that disappeared (tab closed, or heartbeat timeout).
#[wasm_bindgen]
pub fn remove_peer(peer_id: &str) {
    let mut guard = NODE.lock().unwrap();
    if let Some(ref mut node) = *guard {
        node.remove_peer(peer_id);
    }
}

/// Get outgoing messages for JS to send via BroadcastChannel.
/// Returns JSON array of OutgoingMessage.
#[wasm_bindgen]
pub fn drain_outgoing() -> String {
    let mut guard = OUTGOING.lock().unwrap();
    let msgs: Vec<OutgoingMessage> = guard.drain(..).collect();
    serde_json::to_string(&msgs).unwrap_or_else(|_| "[]".into())
}

/// Pop recent visual events.
#[wasm_bindgen]
pub fn drain_events() -> String {
    let mut guard = RECENT_EVENTS.lock().unwrap();
    let events: Vec<DemoEvent> = guard.drain(..).collect();
    serde_json::to_string(&events).unwrap_or_else(|_| "[]".into())
}

/// Simple LCG random: returns f32 in [0, 1).
fn lcg_random(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
    ((*state >> 9) & 0x7FFFFF) as f32 / 8388608.0
}

// ─── DemoNode Implementation ────────────────────────────────────────────────

impl DemoNode {
    fn new(id: &str, rng: &mut u32, seeds: Vec<f32>) -> Self {
        let mut neurons = Vec::with_capacity(NEURON_COUNT);
        let mut synapses = Vec::new();

        // Create neurons in a rough circle
        for i in 0..NEURON_COUNT {
            let angle = (i as f32 / NEURON_COUNT as f32) * std::f32::consts::TAU;
            let radius = 80.0 + lcg_random(rng) * 40.0;
            neurons.push(Neuron {
                id: i as u32,
                activation: lcg_random(rng) * 0.3,
                bias: (lcg_random(rng) - 0.5) * 0.5,
                x: 200.0 + angle.cos() * radius,
                y: 200.0 + angle.sin() * radius,
                vx: 0.0,
                vy: 0.0,
            });
        }

        // Create sparse connections (each neuron connects to ~2-3 others)
        for i in 0..NEURON_COUNT {
            let targets = match i {
                0 => vec![1, 2],
                1 => vec![2, 3],
                2 => vec![3, 0, 4],
                3 => vec![4, 5],
                4 => vec![5, 0],
                5 => vec![1, 4],
                _ => vec![],
            };
            for &t in &targets {
                if t < NEURON_COUNT {
                    synapses.push(Synapse {
                        from: i as u32,
                        to: t as u32,
                        weight: (lcg_random(rng) - 0.5) * 2.0,
                    });
                }
            }
        }

        DemoNode {
            id: id.to_string(),
            tick: 0,
            neurons,
            synapses,
            peers: HashMap::new(),
            peer_id_counter: 0,
            rng: *rng,
            packets_sent: 0,
            packets_recv: 0,
            ticks_per_sec: 60.0,
            last_tick_time: 0.0,
            packets_in_flight: Vec::new(),
        }
    }

    fn tick(&mut self, dt_ms: f64) -> RenderState {
        self.tick += 1;
        self.ticks_per_sec = if dt_ms > 0.0 { 1000.0 / dt_ms } else { 60.0 };

        // 1. Process peer timeouts
        self.check_peer_timeouts();

        // 2. Neural computation (forward pass)
        self.forward_pass();

        // 3. Hebbian learning
        self.hebbian_update();

        // 4. Generate heartbeat
        if self.tick % HEARTBEAT_INTERVAL_TICKS == 0 {
            let avg_act = self.neurons.iter().map(|n| n.activation).sum::<f32>() / self.neurons.len() as f32;
            let hb = HeartbeatMessage {
                from: self.id.clone(),
                tick: self.tick,
                neuron_count: self.neurons.len(),
                avg_activation: avg_act,
            };
            let body = serde_json::to_string(&hb).unwrap_or_default();
            OUTGOING.lock().unwrap().push(OutgoingMessage {
                channel: "broadcast".into(),
                msg_type: "heartbeat".into(),
                body,
            });
        }

        // 5. Exchange gradients with peers periodically
        if self.tick % GRADIENT_EXCHANGE_INTERVAL == 0 {
            for (peer_id, _peer) in &self.peers.clone() {
                let msg = GradientMessage {
                    from: self.id.clone(),
                    tick: self.tick,
                    activations: self.neurons.iter().map(|n| n.activation).collect(),
                };
                let body = serde_json::to_string(&msg).unwrap_or_default();
                OUTGOING.lock().unwrap().push(OutgoingMessage {
                    channel: peer_id.clone(),
                    msg_type: "gradient".into(),
                    body,
                });
                self.packets_sent += 1;
                RECENT_EVENTS.lock().unwrap().push(DemoEvent::PacketSent { to: peer_id.clone() });
                // Add visual packet
                self.packets_in_flight.push(PacketAnim {
                    id: self.tick as u32,
                    peer_id: peer_id.clone(),
                    direction: "out".into(),
                    progress: 0.0,
                });
            }
        }

        // 6. Update packet animations
        for pkt in &mut self.packets_in_flight {
            pkt.progress += 0.03;
        }
        self.packets_in_flight.retain(|p| p.progress < 1.0);

        // 7. Neuron layout relaxation (simple spring forces)
        self.relax_layout();

        // 8. Build render state
        self.build_render_state()
    }

    fn forward_pass(&mut self) {
        // Compute new activations: tanh(sum(inputs * weights) + bias)
        let mut new_acts = vec![0.0f32; self.neurons.len()];

        for syn in &self.synapses {
            let pre_act = self.neurons[syn.from as usize].activation;
            new_acts[syn.to as usize] += pre_act * syn.weight.max(0.0); // only excitatory
        }

        // Add peer influence (peer activations weighted by connection strength)
        for peer in self.peers.values() {
            if peer.alive && !peer.activations.is_empty() {
                // Inject peer's average activation as signal
                let avg = peer.activations.iter().sum::<f32>() / peer.activations.len() as f32;
                // Random neuron gets peer influence
                let target = (peer.id.len() as u32) % self.neurons.len() as u32;
                new_acts[target as usize] += avg * 0.3;
            }
        }

        for (i, neuron) in self.neurons.iter_mut().enumerate() {
            let sum = new_acts[i] + neuron.bias;
            let new_act = sum.tanh();
            // Smooth update
            neuron.activation = neuron.activation * 0.8 + new_act * 0.2;
            neuron.activation = neuron.activation.clamp(0.0, 1.0);

            // Firing event
            if neuron.activation > 0.8 && lcg_random(&mut self.rng) < 0.1 {
                RECENT_EVENTS.lock().unwrap().push(DemoEvent::NeuronFired { id: neuron.id });
            }
        }
    }

    fn hebbian_update(&mut self) {
        for syn in &mut self.synapses {
            let pre = self.neurons[syn.from as usize].activation;
            let post = self.neurons[syn.to as usize].activation;
            let delta = HEBBIAN_ETA * (pre * post - HEBBIAN_LAMBDA * syn.weight);
            syn.weight += delta;
            syn.weight = syn.weight.clamp(-2.0, 2.0);
        }
    }

    fn handle_message(&mut self, msg: &str) {
        let parsed: serde_json::Value = match serde_json::from_str(msg) {
            Ok(v) => v,
            Err(_) => return,
        };

        let from = parsed.get("from").and_then(|v| v.as_str()).unwrap_or("");
        let msg_type = parsed.get("msg_type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "heartbeat" => {
                if !from.is_empty() && from != self.id {
                    let avg_act = parsed.get("avg_activation").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let ncount = parsed.get("neuron_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                    if let Some(peer) = self.peers.get_mut(from) {
                        // Existing peer — update
                        peer.last_seen_tick = self.tick;
                        peer.alive = true;
                        peer.latency_ticks = peer.latency_ticks.saturating_add(1);
                    } else {
                        // New peer discovered!
                        let seed_val = self.tick as f32 * 0.01;
                        let angle = self.peers.len() as f32 * 1.5;
                        let peer = Peer {
                            id: from.to_string(),
                            latency_ticks: 1,
                            last_seen_tick: self.tick,
                            alive: true,
                            activations: vec![avg_act; ncount.max(1)],
                            x: 400.0 + angle.cos() * 60.0,
                            y: 150.0 + angle.sin() * 30.0,
                        };
                        self.peers.insert(from.to_string(), peer);
                        self.peer_id_counter += 1;
                        RECENT_EVENTS.lock().unwrap().push(DemoEvent::PeerDiscovered {
                            id: from.to_string(),
                        });
                        self.packets_recv += 1;
                    }
                }
            }
            "gradient" => {
                if !from.is_empty() && from != self.id {
                    if let Some(peer) = self.peers.get_mut(from) {
                        peer.last_seen_tick = self.tick;
                        peer.alive = true;
                        if let Some(acts) = parsed.get("activations").and_then(|v| v.as_array()) {
                            peer.activations = acts.iter()
                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                .collect();
                        }
                        self.packets_recv += 1;
                        RECENT_EVENTS.lock().unwrap().push(DemoEvent::PacketRecv {
                            from: from.to_string(),
                        });
                        self.packets_in_flight.push(PacketAnim {
                            id: self.tick as u32,
                            peer_id: from.to_string(),
                            direction: "in".into(),
                            progress: 0.0,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn remove_peer(&mut self, peer_id: &str) {
        if self.peers.remove(peer_id).is_some() {
            RECENT_EVENTS.lock().unwrap().push(DemoEvent::PeerLost {
                id: peer_id.to_string(),
            });
        }
    }

    fn check_peer_timeouts(&mut self) {
        let mut dead: Vec<String> = Vec::new();
        for (id, peer) in &self.peers {
            if self.tick - peer.last_seen_tick > PEER_TIMEOUT_TICKS && peer.alive {
                dead.push(id.clone());
            }
        }
        for id in dead {
            if let Some(peer) = self.peers.get_mut(&id) {
                peer.alive = false;
            }
            RECENT_EVENTS.lock().unwrap().push(DemoEvent::PeerLost { id: id.clone() });
        }
    }

    fn relax_layout(&mut self) {
        // Simple spring forces between connected neurons
        for syn in &self.synapses {
            let i = syn.from as usize;
            let j = syn.to as usize;
            if i >= self.neurons.len() || j >= self.neurons.len() { continue; }
            let (x1, y1) = (self.neurons[i].x, self.neurons[i].y);
            let (x2, y2) = (self.neurons[j].x, self.neurons[j].y);
            let dx = x2 - x1;
            let dy = y2 - y1;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let target = 70.0;
            let force = (dist - target) * 0.002;
            self.neurons[i].vx += dx / dist * force;
            self.neurons[i].vy += dy / dist * force;
            self.neurons[j].vx -= dx / dist * force;
            self.neurons[j].vy -= dy / dist * force;
        }

        // Center gravity
        for neuron in &mut self.neurons {
            let dx = 200.0 - neuron.x;
            let dy = 200.0 - neuron.y;
            neuron.vx += dx * 0.001;
            neuron.vy += dy * 0.001;
            neuron.vx *= 0.95;
            neuron.vy *= 0.95;
            neuron.x += neuron.vx;
            neuron.y += neuron.vy;
            neuron.x = neuron.x.clamp(30.0, 370.0);
            neuron.y = neuron.y.clamp(30.0, 370.0);
        }
    }

    fn build_render_state(&self) -> RenderState {
        // Position peers based on alive/dead
        let peer_connections: Vec<PeerRender> = self.peers.values().map(|p| {
            PeerRender {
                id: p.id.clone(),
                alive: p.alive,
                latency_ticks: p.latency_ticks,
                x: p.x,
                y: p.y,
                avg_activation: p.activations.iter().sum::<f32>() / p.activations.len().max(1) as f32,
            }
        }).collect();

        let avg_act = self.neurons.iter().map(|n| n.activation).sum::<f32>() / self.neurons.len() as f32;

        RenderState {
            neuron_id: self.id.clone(),
            tick: self.tick,
            neurons: self.neurons.clone(),
            synapses: self.synapses.clone(),
            peers: peer_connections,
            packets: self.packets_in_flight.clone(),
            stats: StatsState {
                tick: self.tick,
                peers_alive: self.peers.values().filter(|p| p.alive).count(),
                peers_total: self.peers.len(),
                neurons: self.neurons.len(),
                packets_sent: self.packets_sent,
                packets_recv: self.packets_recv,
                tick_rate: self.ticks_per_sec,
                avg_activation: avg_act,
            },
        }
    }
}

// ─── Render State ───────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct RenderState {
    neuron_id: String,
    tick: u64,
    neurons: Vec<Neuron>,
    synapses: Vec<Synapse>,
    peers: Vec<PeerRender>,
    packets: Vec<PacketAnim>,
    stats: StatsState,
}

#[derive(Clone, Serialize)]
struct PeerRender {
    id: String,
    alive: bool,
    latency_ticks: u32,
    x: f32,
    y: f32,
    avg_activation: f32,
}

#[derive(Clone, Serialize)]
struct StatsState {
    tick: u64,
    peers_alive: usize,
    peers_total: usize,
    neurons: usize,
    packets_sent: u64,
    packets_recv: u64,
    tick_rate: f64,
    avg_activation: f32,
}
