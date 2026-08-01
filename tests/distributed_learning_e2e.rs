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
//! 6. The same seeds reproduce the same weight change.
//!
//! Determinism design:
//! - `EngineLoop::new` binds the UDP socket, so the activation frame is
//!   sent to B's bound socket BEFORE B's run loop starts — the kernel
//!   buffers it and B processes it at exactly tick 1.
//! - `EngineConfig::max_ticks` bounds B's run to a fixed number of ticks,
//!   so the weight is a deterministic function of the config, not of
//!   wall-clock scheduling.
//! - The sender's raw socket is rebound by Node A (same port), so B's
//!   gossip reply targets the address A actually listens on.

use neuron_wire::components::{
    ActivationComponent, ActivationMap, EntityId, SynapseComponent, SynapseMap,
};
use neuron_wire::engine_loop::{EngineConfig, EngineLoop};
use neuron_wire::forward_pass::ForwardPassSystem;
use neuron_wire::hebbian::HebbianLearningSystem;
use neuron_wire::ml::MLSystem;
use neuron_wire::neurogenesis::NeurogenesisSystem;
use std::thread;

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
fn make_config(port: u16, seed: [u8; 32], max_ticks: Option<u64>) -> EngineConfig {
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
        security_enabled: false, // focus the test on the learning path
        encrypt_payloads: false,
        stun_enabled: false,
        stun_server: String::new(),
        peer_cache_path: None,
        trust_cache_path: None,
        seed_domain: String::new(),
        max_peers: 100,
        heartbeat_interval_ticks: 0, // disabled for test speed
        per_ip_max_peers: 10,
        trust_enabled: false,
        aging_enabled: true,
        apoptosis_enabled: true,
        neurogenesis_enabled: false, // deterministic: no rand-based spawning
        random_discovery: false,
        static_topology: false,
        packet_loss_rate: 0.0,
        sim_seed: 0,
        max_ticks,
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

/// Serialize a gossip frame carrying TWO synapse entries — one activating
/// B's local neuron (`b_neuron`) and one activating `a_neuron`. Both with
/// gradient magnitude 0.8 (the activation value B will mirror).
fn build_activation_frame() -> Vec<u8> {
    let mut body = Vec::with_capacity(34 + 2 * (32 + 2 + 32 + 4 + 4));
    body.extend_from_slice(&a_neuron().0); // source_entity (who fired)
    body.extend_from_slice(&2u16.to_le_bytes()); // num_synapses = 2

    // Entry 1: b_neuron (B's local neuron) receives activation 0.8.
    body.extend_from_slice(&b_neuron().0); // post_id
    body.extend_from_slice(&1u16.to_le_bytes()); // num_targets
    body.extend_from_slice(&a_neuron().0); // target entity
    body.extend_from_slice(&1.0f32.to_le_bytes()); // weight
    body.extend_from_slice(&0.8f32.to_le_bytes()); // accumulated gradient

    // Entry 2: a_neuron receives activation 0.8.
    body.extend_from_slice(&a_neuron().0); // post_id
    body.extend_from_slice(&1u16.to_le_bytes()); // num_targets
    body.extend_from_slice(&[7u8; 32]); // target entity (opaque)
    body.extend_from_slice(&1.0f32.to_le_bytes()); // weight
    body.extend_from_slice(&0.8f32.to_le_bytes()); // accumulated gradient

    neuron_wire::header::build_frame(neuron_wire::types::MsgType::Data as u8, body, 0)
}

/// Result of one full scenario run.
struct RunResult {
    b_recv_frames: u64,
    a_recv_frames: u64,
    b_weight: f32,
    b_tick: u64,
    b_activation: f32,
}

/// Run the distributed learning scenario once with fixed ports:
/// A sends an activation → B learns → B gossips back → A receives.
fn run_scenario(port_a: u16, port_b: u16) -> RunResult {
    // Node B: brain attached (local neuron with a synapse onto A's neuron).
    // `new()` binds B's socket immediately.
    let config_b = make_config(port_b, seed_b(), Some(400));
    let (mut engine_b, _outbound_tx_b, _events_b) =
        EngineLoop::new(config_b).expect("Node B construct");

    let mut act_b: ActivationMap = std::collections::HashMap::new();
    act_b.insert(
        b_neuron(),
        ActivationComponent {
            value: 1.0, // B's neuron starts active
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
        // lr=0.02, weight_decay=1.0 (no decay → weight strictly grows and
        // can never be micro-pruned), prune=0.001, gossip every 5 ticks.
        HebbianLearningSystem::new(0.02, 1.0, 0.001, 5),
        MLSystem::new(),
        b_neuron(),
    );

    // Pre-send the activation frame to B's ALREADY-BOUND socket. The kernel
    // buffers it; B's run loop receives it at exactly tick 1. The sender
    // binds port_a so that Node A can later take over that exact address.
    {
        let sender = std::net::UdpSocket::bind(format!("127.0.0.1:{}", port_a))
            .expect("sender binds port_a");
        let frame = build_activation_frame();
        sender
            .send_to(&frame, format!("127.0.0.1:{}", port_b))
            .expect("pre-send activation frame to B");
        // Drop the sender: port_a is freed for Node A to bind.
    }

    // Node A: no brain needed (ingress decode path is brain-independent).
    // Outlives B so it is guaranteed to be listening for B's gossip reply.
    let config_a = make_config(port_a, seed_a(), Some(800));
    let (mut engine_a, _outbound_tx_a, _events_a) =
        EngineLoop::new(config_a).expect("Node A construct");

    let handle_a = thread::spawn(move || {
        engine_a.run();
        engine_a
    });
    let handle_b = thread::spawn(move || {
        engine_b.run();
        engine_b
    });

    // B exits after 400 ticks; A exits after 800. Join both.
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
        b_tick: engine_b.tick_for_test(),
        b_activation: engine_b.activation_for_test(&b_neuron()).unwrap_or(-2.0),
    }
}

#[test]
fn distributed_learning_activation_flows_and_learns() {
    let r = run_scenario(9730, 9731);

    // 2. Node B received A's activation frame (at tick 1).
    assert!(
        r.b_recv_frames >= 1,
        "Node B must receive Node A's activation frame (got {})",
        r.b_recv_frames
    );

    // 3. Node B's synapse weight changed (0.5 → >0.6: learned).
    assert!(
        r.b_weight > 0.6,
        "Node B must update its synapse from remote activation (weight={}, tick={}, b_act={}, b_recv={}, a_recv={})",
        r.b_weight,
        r.b_tick,
        r.b_activation,
        r.b_recv_frames,
        r.a_recv_frames
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
    // Run the identical scenario twice with identical seeds on distinct ports.
    let r1 = run_scenario(9740, 9741);
    let r2 = run_scenario(9750, 9751);

    // Both runs: learning happened.
    assert!(r1.b_weight > 0.6, "run 1 learned (weight={})", r1.b_weight);
    assert!(r2.b_weight > 0.6, "run 2 learned (weight={})", r2.b_weight);

    // Same seed + same tick budget + frame at tick 1 → the execution is
    // bit-deterministic (no rand in the learning path), so the weights
    // must match closely. Allow a tiny f32 tolerance.
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
        "run 1 full loop (b={}, a={})",
        r1.b_recv_frames,
        r1.a_recv_frames
    );
    assert!(
        r2.b_recv_frames >= 1 && r2.a_recv_frames >= 1,
        "run 2 full loop (b={}, a={})",
        r2.b_recv_frames,
        r2.a_recv_frames
    );
}
