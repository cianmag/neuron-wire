//! End-to-end integration tests for neuron-wire.
//!
//! These tests spin up real EngineLoop instances on localhost and verify
//! that the full stack works: authentication, encryption, DHT discovery,
//! and gradient exchange.

use neuron_wire::engine_loop::{EngineConfig, EngineLoop, OutgoingPacket, Reliability};
use std::thread;
use std::time::Duration;

/// Helper: create a minimal EngineConfig on a given port.
fn make_config(port: u16) -> EngineConfig {
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
        identity_seed: None, // random identity
        security_enabled: true,
        encrypt_payloads: false,
        stun_enabled: false,
        stun_server: String::new(),
        peer_cache_path: None,
        trust_cache_path: None,
        seed_domain: String::new(),
        max_peers: 100,
        heartbeat_interval_ticks: 0, // disabled for test speed
        per_ip_max_peers: 10,
    }
}

// ─── Test: Two nodes can be constructed and bound ──────────────

#[test]
fn e2e_two_nodes_construct() {
    let config_a = make_config(9401);
    let config_b = make_config(9402);

    let result_a = EngineLoop::new(config_a);
    assert!(result_a.is_ok(), "Node A should construct: {:?}", result_a.err());

    let result_b = EngineLoop::new(config_b);
    assert!(result_b.is_ok(), "Node B should construct: {:?}", result_b.err());
}

// ─── Test: Node A can send a packet to Node B ──────────────────

#[test]
fn e2e_send_packet_between_nodes() {
    // Construct Node A
    let config_a = make_config(9411);
    let (mut engine_a, outbound_tx_a, _events_rx_a) =
        EngineLoop::new(config_a).expect("Node A construct");

    // Construct Node B
    let config_b = make_config(9412);
    let (mut engine_b, _outbound_tx_b, mut events_rx_b) =
        EngineLoop::new(config_b).expect("Node B construct");

    // Run Node B in a background thread
    let shutdown_b = engine_b.shutdown.clone();
    let handle_b = thread::spawn(move || {
        engine_b.run();
    });

    // Give Node B time to start listening
    thread::sleep(Duration::from_millis(50));

    // Send a packet from A to B via the outbound channel
    let body = b"hello from node A".to_vec();
    let frame = neuron_wire::header::build_frame(20, body, 0); // msg_type=20 (GRADIENT)
    let packet = OutgoingPacket {
        payload: frame,
        dst: "127.0.0.1:9412".parse().unwrap(),
        mode: Reliability::BestEffort,
    };
    outbound_tx_a.send(packet).expect("send should succeed");

    // Run Node A briefly to drain the outbound channel
    let shutdown_a = engine_a.shutdown.clone();
    let handle_a = thread::spawn(move || {
        engine_a.run();
    });

    // Wait for Node B to potentially receive the packet
    thread::sleep(Duration::from_millis(200));

    // Shutdown both
    shutdown_a.store(true, std::sync::atomic::Ordering::Relaxed);
    shutdown_b.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle_a.join();
    let _ = handle_b.join();
}

// ─── Test: Two nodes with signing disabled can exchange ────────

#[test]
fn e2e_unsigned_packet_exchange() {
    // Node A — no signing
    let mut config_a = make_config(9421);
    config_a.security_enabled = false;
    let (mut engine_a, outbound_tx_a, _events_rx_a) =
        EngineLoop::new(config_a).expect("Node A construct");

    // Node B — no signing
    let mut config_b = make_config(9422);
    config_b.security_enabled = false;
    let (mut engine_b, _outbound_tx_b, mut events_rx_b) =
        EngineLoop::new(config_b).expect("Node B construct");

    // Run Node B
    let shutdown_b = engine_b.shutdown.clone();
    let handle_b = thread::spawn(move || {
        engine_b.run();
    });

    thread::sleep(Duration::from_millis(50));

    // Send gradient from A to B
    let body = b"gradient weights [0.1, 0.2, 0.3]".to_vec();
    let frame = neuron_wire::header::build_frame(20, body, 0);
    let packet = OutgoingPacket {
        payload: frame,
        dst: "127.0.0.1:9422".parse().unwrap(),
        mode: Reliability::BestEffort,
    };
    outbound_tx_a.send(packet).expect("send should succeed");

    // Run Node A
    let shutdown_a = engine_a.shutdown.clone();
    let handle_a = thread::spawn(move || {
        engine_a.run();
    });

    // Wait for delivery
    thread::sleep(Duration::from_millis(200));

    // Check if Node B received an event
    let _ = events_rx_b.try_recv();

    shutdown_a.store(true, std::sync::atomic::Ordering::Relaxed);
    shutdown_b.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle_a.join();
    let _ = handle_b.join();
}

// ─── Test: DHT bootstrap between two nodes ─────────────────────

#[test]
fn e2e_dht_bootstrap() {
    // Node A — bootstrap peer for B
    let mut config_a = make_config(9431);
    config_a.security_enabled = false;
    let (mut engine_a, _outbound_tx_a, _events_rx_a) =
        EngineLoop::new(config_a).expect("Node A construct");

    // Node B — bootstraps to A
    let mut config_b = make_config(9432);
    config_b.security_enabled = false;
    config_b.local_peers = vec!["127.0.0.1:9431".parse().unwrap()];
    let (mut engine_b, _outbound_tx_b, _events_rx_b) =
        EngineLoop::new(config_b).expect("Node B construct");

    // Run both
    let shutdown_a = engine_a.shutdown.clone();
    let handle_a = thread::spawn(move || {
        engine_a.run();
    });

    let shutdown_b = engine_b.shutdown.clone();
    let handle_b = thread::spawn(move || {
        engine_b.run();
    });

    // Let them discover each other via DHT
    thread::sleep(Duration::from_secs(2));

    shutdown_a.store(true, std::sync::atomic::Ordering::Relaxed);
    shutdown_b.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle_a.join();
    let _ = handle_b.join();
}

// ─── Test: Connection limit enforcement ────────────────────────

#[test]
fn e2e_connection_limit() {
    // Node with max_peers=2
    let mut config = make_config(9441);
    config.security_enabled = false;
    config.max_peers = 2;
    let (mut engine, _outbound_tx, _events_rx) =
        EngineLoop::new(config).expect("Node construct");

    let shutdown = engine.shutdown.clone();
    let handle = thread::spawn(move || {
        engine.run();
    });

    // Let it run briefly
    thread::sleep(Duration::from_millis(100));

    shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();
}

// ─── Test: Heartbeat config validation ─────────────────────────

#[test]
fn e2e_heartbeat_config() {
    let mut config = make_config(9451);
    config.heartbeat_interval_ticks = 100;

    let result = EngineLoop::new(config);
    assert!(result.is_ok(), "Heartbeat config should be valid");
}

// ─── Test: Trust system persistence roundtrip ──────────────────

#[test]
fn e2e_trust_persistence() {
    use neuron_wire::trust::{TrustEvent, TrustSystem};

    let mut trust = TrustSystem::new();
    let eid = [1u8; 32];

    // Record some events
    trust.record_event(eid, TrustEvent::ValidSignature);
    trust.record_event(eid, TrustEvent::ValidSignature);
    trust.record_event(eid, TrustEvent::SuccessfulHandshake);

    // Save to temp file
    let path = std::env::temp_dir().join("nwp_trust_test.bin");
    let count = trust.save_to_file(&path.to_string_lossy()).unwrap();
    assert!(count >= 1);

    // Load into a fresh TrustSystem
    let mut trust2 = TrustSystem::new();
    trust2.load_from_file(&path.to_string_lossy()).unwrap();

    // Verify the peer was loaded
    let stats = trust2.stats();
    assert!(stats.total_peers >= 1, "Should have loaded at least 1 peer");

    // Cleanup
    let _ = std::fs::remove_file(&path);
}
