//! Profiling harness for neuron-wire.
//!
//! Run with: `cargo test --test profiling -- --nocapture --ignored`
//!
//! These tests are designed to be run under perf, flamegraph, or Instruments
//! to identify hot paths and optimize performance.

use std::time::{Duration, Instant};

use neuron_wire::header;

// ── Benchmark: Header build/parse roundtrip ──────────────────

/// Measure header build + parse throughput (ops/sec).
/// Use under `cargo flamegraph --test profiling` to see hot paths.
#[test]
#[ignore]
fn bench_header_roundtrip() {
    let iterations = 100_000;
    let body = vec![0xABu8; 128];

    let start = Instant::now();
    for _ in 0..iterations {
        let frame = header::build_frame(5, body.clone(), 0);
        let _ = header::parse_frame(&frame[4..]).unwrap();
    }
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    eprintln!(
        "📊 header_roundtrip: {} iterations in {:.2?} ({:.0} ops/sec)",
        iterations, elapsed, ops_per_sec
    );
    eprintln!("   Per operation: {:.2?}", elapsed / iterations);
}

// ── Benchmark: Large body serialization ──────────────────────

/// Measure serialization throughput for various body sizes.
#[test]
#[ignore]
fn bench_body_sizes() {
    let sizes = [64, 256, 1024, 4096, 16384, 65536];
    let iterations = 10_000;

    for &size in &sizes {
        let body = vec![0u8; size];
        let start = Instant::now();
        for _ in 0..iterations {
            let frame = header::build_frame(5, body.clone(), 0);
            let _ = header::parse_frame(&frame[4..]).unwrap();
        }
        let elapsed = start.elapsed();
        let throughput_mbps =
            (size as f64 * iterations as f64) / elapsed.as_secs_f64() / 1_000_000.0;

        eprintln!(
            "📊 body_{:5}B: {:.2?} total, {:.1} MB/s, {:.0} ops/sec",
            size,
            elapsed,
            throughput_mbps,
            iterations as f64 / elapsed.as_secs_f64()
        );
    }
}

// ── Benchmark: UDP send/recv throughput ───────────────────────

/// Measure raw UDP send/recv throughput on loopback.
#[test]
#[ignore]
fn bench_udp_throughput() {
    use std::net::UdpSocket;

    let sock_a = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sock_b = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock_a.set_read_timeout(Some(Duration::from_millis(1))).ok();
    sock_b.set_read_timeout(Some(Duration::from_millis(1))).ok();

    let addr_b = sock_b.local_addr().unwrap();
    let _addr_a = sock_a.local_addr().unwrap();

    let frame = header::build_frame(5, vec![0u8; 256], 0);
    let iterations = 100_000;

    let start = Instant::now();
    for _ in 0..iterations {
        sock_a.send_to(&frame, addr_b).unwrap();
        let mut buf = [0u8; 65535];
        let _ = sock_b.recv_from(&mut buf);
    }
    let elapsed = start.elapsed();

    let total_bytes = frame.len() as f64 * iterations as f64;
    let throughput_mbps = total_bytes / elapsed.as_secs_f64() / 1_000_000.0;

    eprintln!(
        "📊 udp_sendrecv: {} iterations in {:.2?} ({:.0} ops/sec, {:.1} MB/s)",
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64(),
        throughput_mbps
    );
}

// ── Benchmark: Concurrent UDP (multi-sender) ─────────────────

/// Measure UDP throughput with multiple concurrent senders.
#[test]
#[ignore]
fn bench_concurrent_udp() {
    use std::net::UdpSocket;
    use std::thread;

    let num_senders = 4;
    let pkts_per_sender = 25_000;
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    listener
        .set_read_timeout(Some(Duration::from_millis(1)))
        .ok();
    let target = listener.local_addr().unwrap();

    let start = Instant::now();

    // Spawn senders
    let senders: Vec<_> = (0..num_senders)
        .map(|_| {
            let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
            let target = target;
            thread::spawn(move || {
                let frame = header::build_frame(5, vec![0u8; 128], 0);
                for _ in 0..pkts_per_sender {
                    let _ = sock.send_to(&frame, target);
                }
            })
        })
        .collect();

    // Drain receiver
    let mut total_recv = 0u64;
    let mut buf = [0u8; 65535];
    loop {
        if let Ok((n, _)) = listener.recv_from(&mut buf) {
            if n > 0 {
                total_recv += 1;
            }
        } else {
            // Check if all senders are done
            let all_done = senders.iter().all(|s| s.is_finished());
            if all_done {
                break;
            }
        }
    }

    for s in senders {
        s.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_sent = num_senders * pkts_per_sender;

    eprintln!(
        "📊 concurrent_udp: {} senders × {} pkts = {} sent, {} recv in {:.2?}",
        num_senders, pkts_per_sender, total_sent, total_recv, elapsed
    );
    eprintln!(
        "   Throughput: {:.0} pkts/sec ({:.1} MB/s)",
        total_sent as f64 / elapsed.as_secs_f64(),
        (total_sent as f64 * 128.0) / elapsed.as_secs_f64() / 1_000_000.0
    );
}

// ── Benchmark: Memory allocation pattern ─────────────────────

/// Repeatedly allocate and free vectors to test allocator performance.
#[test]
#[ignore]
fn bench_allocator_throughput() {
    let iterations = 100_000;
    let sizes = [64, 256, 1024, 4096];

    for &size in &sizes {
        let start = Instant::now();
        for _ in 0..iterations {
            let v = vec![0u8; size];
            drop(v);
        }
        let elapsed = start.elapsed();

        eprintln!(
            "📊 alloc_{:5}B: {} alloc+free in {:.2?} ({:.0} ops/sec)",
            size,
            iterations,
            elapsed,
            iterations as f64 / elapsed.as_secs_f64()
        );
    }
}

// ── Benchmark: HashMap operations (trust system hot path) ────

/// Measure HashMap operations simulating trust system load.
#[test]
#[ignore]
fn bench_hashmap_trust_pattern() {
    use std::collections::HashMap;

    let iterations = 100_000;
    let mut map: HashMap<u64, f32> = HashMap::with_capacity(1000);

    // Insert phase
    let start = Instant::now();
    for i in 0..1000u64 {
        map.insert(i, 0.5);
    }
    let insert_time = start.elapsed();

    // Lookup + update phase
    let start = Instant::now();
    for _ in 0..iterations {
        for i in 0..1000u64 {
            if let Some(score) = map.get_mut(&i) {
                *score = (*score + 0.05).min(1.0);
            }
        }
    }
    let update_time = start.elapsed();

    eprintln!(
        "📊 hashmap_trust: 1000 inserts in {:.2?}, {} lookups+updates in {:.2?}",
        insert_time, iterations, update_time
    );
    eprintln!("   Per update: {:.2?}", update_time / iterations);
}
