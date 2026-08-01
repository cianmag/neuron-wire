//! End-to-end distributed learning test — the bridge between the
//! "distributed network" and "distributed learning".
//!
//! Proves, with real EngineLoop instances over real UDP sockets:
//!
//! 1. Node A sends an activation frame (NWP `Data` gossip).
//! 2. Node B receives it, decodes it, and feeds it into the live
//!    neural path (activation map → Hebbian STDP).
//! 3. Node B's synapse weight changes (learning happened).
//! 4. Node B sends a learning signal (gradient gossip) back.
//! 5. Node A receives the response.
//! 6. The same seeds reproduce the same learning outcome.

use neuron_wire::components::{
    ActivationComponent, ActivationMap, EntityId, SynapseComponent, SynapseMap,
};
use neuron_wire::engine_loop::{EngineConfig, EngineLoop, OutgoingPacket, Reliability};
use neuron_wire::forward_pass::ForwardPassSystem;
use neuron_wire::hebbian::HebbianLearningSystem;
use neuron_wire::ml::MLSystem;
use neuron_wire::neurogenesis::NeurogenesisSystem;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

/// Deterministic node identity seed (same seed → same Ed25519 identity).
fn seed_a() -> [u8; 32] {
    let mut s = [0u8; 32];
    s[0] = 0xA1;
    s
}
fn seed_b() -> [u8; 32] {
    let mut s = [0u8; 32];
    s[0] = 0xB2;
    s
}

/// EngineConfig for a deterministic test node.
fn make_config(port: u16, seed: [u8; 32]) -> EngineConfig {
    EngineConfig {
        bind_addr: format!("127.0.0.1:{}", port),
        tick_interval_ms: 1,
        retransmit_interval_ticks: 10,
        cleanup_interval_ticks: 1000,
        max_outbound_queue: 10_000,
        recv_buffer_size: 65535,
        gradient_half_life_ms: 100.0,
        local_peers: Vec::new(),
        shared_stats: None,
        freshness_config: None,
        identity_seed: Some(seed),
        security_enabled: false, // focus test on the learning path
        encrypt_payloads: false,
        stun_enabled: false,
        stun_server: String::new(),
        peer_cache_path: None,
        trust_cache_path: None,
        seed_domain: String::new(),
        max_peers: 100,
        heartbeat_interval_ticks: 0,
        per_ip_max_peers: 10,
        trust_enabled: false,
        aging_enabled: true,
        apoptosis_enabled: true,
        neurogenesis_enabled: true,
        random_discovery: false,
        static_topology: false,
        packet_loss_rate: 0.0,
        sim_seed: 0,
    }
}

/// Neuron ids used by the test.
/// B's local neuron (`b_neuron`) has a synapse onto A's neuron (`a_neuron`).
fn a_neuron() -> EntityId {
    let mut b = [0u8; 32];
    b[31] = 1;
    EntityId(b)
}
fn b_neuron() -> EntityId {
    let mut b = [0u8; 32];
    b[31] = 2;
    EntityId(b)
}

/// Serialize a single-synapse gossip frame exactly as the production
/// `serialize_gossip_packet` does (see src/hebbian.rs).
fn build_activation_frame() -> Vec<u8> {
    let mut body = Vec::with_capacity(32 + 2 + 32 + 2 + 32 + 4 + 4);
    body.extend_from_slice(&a_neuron().0); // source_entity (who fired)
    body.extend_from_slice(&1u16.to_le_bytes()); // num_synapses = 1
    body.extend_from_slice(&a_neuron().0); // post_id
    body.extend_from_slice(&1u16.to_le_bytes()); // num_targets = 1
    body.extend_from_slice(&[7u8; 32]); // target entity (opaque)
    body.extend_from_slice(&1.0f32.to_le_bytes()); // weight
    body.extend_from_slice(&0.5f32.to_le_bytes()); // accumulated gradient (activation magnitude)
    neuron_wire::header::build_frame(neuron_wire::types::MsgType::Data as u8, body, 0)
}

/// Result of one full scenario run.
struct RunResult {
    b_recv_frames: u64,
    a_recv_frames: u64,
    b_weight: f32,
}

/// Run the distributed learning scenario once:
/// A sends an activation → B learns → B gossips back → A receives.
fn run_scenario(port_a: u16, port_b: u16) -> RunResult {
    // Node B: brain attached (local neuron with a synapse onto A's neuron).
    let config_b = make_config(port_b, seed_b());
    let (mut engine_b, _outbound_tx_b, _events_b) =
        EngineLoop::new(config_b).expect("Node B construct");

    let mut act_b: ActivationMap = std::collections::HashMap::new();
    act_b.insert(
        b_neuron(),
        ActivationComponent {
            value: 1.0, // B's neuron is active
            last_updated_tick: 0,
        },
    );
    let mut syn_b: SynapseMap = std::collections::HashMap::new();
    syn_b.insert(
        b_neuron(),
        SynapseComponent {
            target_entities: vec![a_neuron()], // B listens to A's neuron
            weights: vec![0.5],
            accumulated_gradients: vec![0.0],
        },
    );
    engine_b.attach_brain(
        act_b,
        syn_b,
        ForwardPassSystem::default(),
        NeurogenesisSystem::default(),
        HebbianLearningSystem::new(0.01, 0.999, 0.001, 5), // gossip every 5 ticks
        MLSystem::new(),
        b_neuron(),
    );

    let shutdown_b = engine_b.shutdown.clone();
    let handle_b = thread::spawn(move || {
        engine_b.run();
        engine_b
    });

    // Node A: no brain needed (ingress decode path is brain-independent).
    let config_a = make_config(port_a, seed_a());
    let (mut engine_a, outbound_tx_a, _events_a) =
        EngineLoop::new(config_a).expect("Node A construct");
    let shutdown_a = engine_a.shutdown.clone();
    let handle_a = thread::spawn(move || {
        engine_a.run();
        engine_a
    });

    // Let both sockets bind and start ticking.
    thread::sleep(Duration::from_millis(80));

    // 1. Node A sends an activation frame to Node B.
    let frame = build_activation_frame();
    let packet = OutgoingPacket {
        payload: frame,
        dst: format!("127.0.0.1:{}", port_b).parse().unwrap(),
        mode: Reliability::BestEffort,
    };
    outbound_tx_a.send(packet).expect("A sends activation");

    // 2-5. Wait for B to learn, gossip back, and A to receive.
    thread::sleep(Duration::from_millis(900));

    // Shutdown both and reclaim the engines.
    shutdown_a.store(true, Ordering::Relaxed);
    shutdown_b.store(true, Ordering::Relaxed);
    let engine_a = handle_a.join().expect("A joined");
    let engine_b = handle_b.join().expect("B joined");

    let (b_recv, _b_sent) = engine_b.learning_stats();
    let (a_recv, _a_sent) = engine_a.learning_stats();
    let b_weight = engine_b
        .synapse_weight_for_test(&b_neuron(), &a_neuron())
        .unwrap_or(-1.0);

    RunResult {
        b_recv_frames: b_recv,
        a_recv_frames: a_recv,
        b_weight,
    }
}

#[test]
fn distributed_learning_activation_flows_and_learns() {
    let r = run_scenario(9611, 9612);

    // 2. Node B received A's activation frame.
    assert!(
        r.b_recv_frames >= 1,
        "Node B must receive Node A's activation frame (got {})",
        r.b_recv_frames
    );

    // 3. Node B's synapse weight changed (0.5 → >0.6: learned, and well
    //    above the 0.999^tick decay floor of ~0.22).
    assert!(
        r.b_weight > 0.6,
        "Node B must update its synapse from remote activation (weight={})",
        r.b_weight
    );

    // 4-5. Node B sent a learning signal back and Node A received it.
    assert!(
        r.a_recv_frames >= 1,
        "Node A must receive Node B's learning response (got {})",
        r.a_recv_frames
    );
}

#[test]
fn distributed_learning_same_seed_reproduces_same_weight_change() {
    // Run the identical scenario twice with identical seeds.
    let r1 = run_scenario(9621, 9622);
    let r2 = run_scenario(9631, 9632);

    // Both runs: learning happened.
    assert!(r1.b_weight > 0.6, "run 1 learned (weight={})", r1.b_weight);
    assert!(r2.b_weight > 0.6, "run 2 learned (weight={})", r2.b_weight);

    // Same seed → same weight change (tolerance for ±a few ticks of
    // wall-clock scheduling: STDP is 0.005/tick, so ±10 ticks = ±0.05).
    let diff = (r1.b_weight - r2.b_weight).abs();
    assert!(
        diff < 0.05,
        "same seeds must reproduce the same weight change (w1={}, w2={}, diff={})",
        r1.b_weight,
        r2.b_weight,
        diff
    );

    // Both runs saw the full bidirectional loop.
    assert!(
        r1.b_recv_frames >= 1 && r1.a_recv_frames >= 1,
        "run 1 full loop"
    );
    assert!(
        r2.b_recv_frames >= 1 && r2.a_recv_frames >= 1,
        "run 2 full loop"
    );
}
