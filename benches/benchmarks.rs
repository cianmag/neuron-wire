//! Criterion benchmarks for neuron-wire hot paths.
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::collections::HashMap;

// ─── Header / CRC ───────────────────────────────────────────────

fn bench_header_from_bytes(c: &mut Criterion) {
    let h = neuron_wire::header::MessageHeader::new(3, 64, 0);
    let bytes = h.to_bytes();
    c.bench_function("header_from_bytes", |b| {
        b.iter(|| {
            let parsed = neuron_wire::header::MessageHeader::from_bytes(black_box(&bytes)).unwrap();
            black_box(parsed.msg_type);
        })
    });
}

fn bench_header_to_bytes(c: &mut Criterion) {
    let h = neuron_wire::header::MessageHeader::new(3, 64, 0);
    c.bench_function("header_to_bytes", |b| {
        b.iter(|| {
            black_box(black_box(&h).to_bytes());
        })
    });
}

fn bench_crc32(c: &mut Criterion) {
    let sizes = [0usize, 64, 256, 1024, 4096];
    for &size in &sizes {
        let data = vec![0xABu8; size];
        let mut group = c.benchmark_group(format!("crc32_{}B", size));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function("crc32", |b| {
            b.iter(|| black_box(neuron_wire::crc::crc32(black_box(&data))))
        });
        group.finish();
    }
}

// ─── Frame Build / Parse ────────────────────────────────────────

fn bench_build_frame(c: &mut Criterion) {
    let body_sizes = [0usize, 64, 256, 1024];
    for &size in &body_sizes {
        let body = vec![0x42u8; size];
        c.bench_function(format!("build_frame_{}B", size), |b| {
            b.iter(|| {
                black_box(neuron_wire::header::build_frame(
                    black_box(5),
                    black_box(body.clone()),
                    black_box(0),
                ))
            })
        });
    }
}

fn bench_parse_frame(c: &mut Criterion) {
    for size in [0usize, 64, 256, 1024] {
        let body = vec![0x42u8; size];
        let frame = neuron_wire::header::build_frame(5, body, 0);
        let msg = &frame[4..]; // skip 4-byte len prefix
        c.bench_function(format!("parse_frame_{}B", size), |b| {
            b.iter(|| {
                let (h, b) = neuron_wire::header::parse_frame(black_box(msg)).unwrap();
                black_box((h.msg_type, b.len()));
            })
        });
    }
}

// ─── DHT hot paths ──────────────────────────────────────────────

fn bench_bucket_index(c: &mut Criterion) {
    let local = neuron_wire::dht::NodeId::random();
    let target = neuron_wire::dht::NodeId::random();
    c.bench_function("dht_bucket_index", |b| {
        b.iter(|| black_box(local.bucket_index(black_box(&target))))
    });
}

fn bench_xor_distance(c: &mut Criterion) {
    let a = neuron_wire::dht::NodeId::random();
    let b = neuron_wire::dht::NodeId::random();
    c.bench_function("dht_xor_distance", |b| {
        b.iter(|| black_box(a.xor_distance(black_box(&b))))
    });
}

fn bench_closest_fast(c: &mut Criterion) {
    use neuron_wire::dht::{NodeId, RoutingTable};
    let local_id = NodeId::random();
    let mut rt = RoutingTable::new_for_test(local_id);
    // Insert 256 random entries to populate
    for _ in 0..256 {
        let id = NodeId::random();
        let addr = "127.0.0.1:8080".parse().unwrap();
        rt.insert(id, addr, 100.0);
    }
    let target = NodeId::random();
    c.bench_function("dht_closest_fast", |b| {
        b.iter(|| black_box(rt.closest_fast(black_box(&target))))
    });
}

fn bench_nearest_nodes(c: &mut Criterion) {
    use neuron_wire::dht::{NodeId, RoutingTable};
    let local_id = NodeId::random();
    let mut rt = RoutingTable::new_for_test(local_id);
    for _ in 0..256 {
        let id = NodeId::random();
        let addr = "127.0.0.1:8080".parse().unwrap();
        rt.insert(id, addr, 100.0);
    }
    let target = NodeId::random();
    c.bench_function("dht_nearest_nodes", |b| {
        b.iter(|| black_box(rt.nearest_nodes(black_box(&target), 8)))
    });
}

// ─── Hebbian hot paths ──────────────────────────────────────────

fn bench_hebbian_stdp_update(c: &mut Criterion) {
    use neuron_wire::components::SynapseComponent;
    let mut synapse = SynapseComponent {
        target_entities: vec![neuron_wire::types::EntityId([0u8; 32])],
        weights: vec![0.5],
        accumulated_gradients: vec![0.0],
    };
    c.bench_function("hebbian_stdp_update", |b| {
        b.iter(|| {
            synapse.weights[0] += 0.001 * (1.0 - synapse.weights[0]);
            synapse.weights[0] = synapse.weights[0].clamp(-1.0, 1.0);
            black_box(&synapse.weights[0]);
        })
    });
}

fn bench_hebbian_weight_decay(c: &mut Criterion) {
    use neuron_wire::components::SynapseComponent;
    let mut synapse = SynapseComponent {
        target_entities: vec![neuron_wire::types::EntityId([0u8; 32])],
        weights: vec![0.5],
        accumulated_gradients: vec![0.0],
    };
    c.bench_function("hebbian_weight_decay", |b| {
        b.iter(|| {
            for w in &mut synapse.weights {
                *w *= 0.999;
            }
            black_box(&synapse.weights[0]);
        })
    });
}

fn bench_hebbian_micro_pruning(c: &mut Criterion) {
    use neuron_wire::components::SynapseComponent;
    let mut synapses: HashMap<neuron_wire::types::EntityId, SynapseComponent> = HashMap::new();
    let prune_threshold = 0.001f32;
    for i in 0..100 {
        let mut eid = [0u8; 32];
        eid[31] = i;
        let targets: Vec<_> = (0..10)
            .map(|j| {
                let mut t = [0u8; 32];
                t[31] = j;
                neuron_wire::types::EntityId(t)
            })
            .collect();
        synapses.insert(
            neuron_wire::types::EntityId(eid),
            SynapseComponent {
                target_entities: targets,
                weights: vec![if i % 3 == 0 { 0.0005 } else { 0.5 }; 10],
                accumulated_gradients: vec![0.0; 10],
            },
        );
    }
    c.bench_function("hebbian_micro_pruning_100", |b| {
        b.iter_batched(
            || synapses.clone(),
            |mut syns| {
                let dead: Vec<_> = syns
                    .iter()
                    .filter(|(_, s)| s.weights.iter().all(|w| *w < prune_threshold))
                    .map(|(id, _)| *id)
                    .collect();
                for id in dead {
                    syns.remove(&id);
                }
                black_box(syns.len());
            },
            BatchSize::SmallInput,
        )
    });
}

// ─── Forward Pass hot paths ─────────────────────────────────────

fn bench_forward_pass_tick_small(c: &mut Criterion) {
    use neuron_wire::components::*;
    use neuron_wire::forward_pass::ForwardPassSystem;
    use neuron_wire::neurogenesis::NeurogenesisSystem;
    use std::collections::HashMap;

    let mut fp = ForwardPassSystem::new(0.9, 0.1);
    let mut neuro = NeurogenesisSystem::new(3.0, 0.99);
    let mut activations: ActivationMap = HashMap::new();
    let mut synapses: SynapseMap = HashMap::new();

    // 10 neurons fully connected
    for i in 0..10u8 {
        let mut eid = [0u8; 32];
        eid[31] = i;
        let id = EntityId(eid);
        activations.insert(
            id,
            ActivationComponent {
                value: 0.5,
                last_updated_tick: 0,
            },
        );
        let targets: Vec<_> = (0..10)
            .filter(|&j| j != i)
            .map(|j| {
                let mut t = [0u8; 32];
                t[31] = j;
                EntityId(t)
            })
            .collect();
        synapses.insert(
            id,
            SynapseComponent {
                target_entities: targets,
                weights: vec![0.5; 9],
                accumulated_gradients: vec![0.0; 9],
            },
        );
    }

    let observations = HashMap::new();
    c.bench_function("forward_pass_tick_10n", |b| {
        b.iter(|| {
            let _r = fp.tick(
                black_box(&mut activations),
                black_box(&mut synapses),
                black_box(&mut neuro),
                black_box(1),
                black_box(&observations),
            );
        })
    });
}

fn bench_forward_pass_tick_50n(c: &mut Criterion) {
    use neuron_wire::components::*;
    use neuron_wire::forward_pass::ForwardPassSystem;
    use neuron_wire::neurogenesis::NeurogenesisSystem;
    use std::collections::HashMap;

    let mut fp = ForwardPassSystem::new(0.9, 0.1);
    let mut neuro = NeurogenesisSystem::new(3.0, 0.99);
    let mut activations: ActivationMap = HashMap::new();
    let mut synapses: SynapseMap = HashMap::new();

    let n = 50u8;
    let k = 10; // connections per neuron
    for i in 0..n {
        let mut eid = [0u8; 32];
        eid[31] = i;
        let id = EntityId(eid);
        activations.insert(
            id,
            ActivationComponent {
                value: 0.5,
                last_updated_tick: 0,
            },
        );
        let targets: Vec<_> = (0..k)
            .map(|j| {
                let mut t = [0u8; 32];
                t[31] = (i + j) % n;
                EntityId(t)
            })
            .collect();
        synapses.insert(
            id,
            SynapseComponent {
                target_entities: targets,
                weights: vec![0.5; k],
                accumulated_gradients: vec![0.0; k],
            },
        );
    }

    let observations = HashMap::new();
    c.bench_function("forward_pass_tick_50n_x10k", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _r = fp.tick(
                    black_box(&mut activations),
                    black_box(&mut synapses),
                    black_box(&mut neuro),
                    black_box(1),
                    black_box(&observations),
                );
            }
        })
    });
}

criterion_group!(
    benches,
    bench_header_from_bytes,
    bench_header_to_bytes,
    bench_crc32,
    bench_build_frame,
    bench_parse_frame,
    bench_bucket_index,
    bench_xor_distance,
    bench_closest_fast,
    bench_nearest_nodes,
    bench_hebbian_stdp_update,
    bench_hebbian_weight_decay,
    bench_hebbian_micro_pruning,
    bench_forward_pass_tick_small,
    bench_forward_pass_tick_50n,
);
criterion_main!(benches);
