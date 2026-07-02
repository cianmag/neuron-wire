use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Simple ML system benchmark simulating one tick with N neurons and M synapses.
fn bench_ml_tick_n1000(c: &mut Criterion) {
    // This benchmark requires the `criterion` dev-dependency to be added to Cargo.toml:
    //   [dev-dependencies]
    //   criterion = { version = "0.5", features = ["html_reports"] }
    //   [[bench]]
    //   name = "bench_ml"
    //   harness = false

    use neuron_wire::components::{
        ActivationComponent, ActivationMap, EntityId, SynapseComponent, SynapseMap,
    };
    use neuron_wire::ml::MLSystem;

    let mut ml = MLSystem::new();
    let mut activations = ActivationMap::new();
    let mut synapses = SynapseMap::new();
    let observations = vec![];

    // Build a graph of 1000 neurons, each with ~100 synapses
    let n_neurons = 1000usize;
    let syn_per_neuron = 100usize;

    // Create entities
    let entities: Vec<EntityId> = (0..n_neurons)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            EntityId(id)
        })
        .collect();

    // Populate activations
    for e in &entities {
        activations.insert(
            *e,
            ActivationComponent {
                value: ((e.0[0] as f32) / n_neurons as f32 - 0.5) * 2.0,
                last_updated_tick: 0,
            },
        );
    }

    // Populate synapses (each neuron connects to ~100 random targets)
    for (i, e) in entities.iter().enumerate() {
        let targets: Vec<EntityId> = entities
            .iter()
            .cycle()
            .skip(i + 1)
            .take(syn_per_neuron)
            .copied()
            .collect();
        let weights = targets
            .iter()
            .map(|_| rand::random::<f32>() * 2.0 - 1.0)
            .collect();
        let grads = targets
            .iter()
            .map(|_| rand::random::<f32>() * 0.1)
            .collect();

        synapses.insert(
            *e,
            SynapseComponent {
                target_entities: targets,
                weights,
                accumulated_gradients: grads,
            },
        );
    }

    let tick: u64 = 1;

    c.bench_function("ml_tick_1000x100", |b| {
        b.iter(|| {
            let _report = ml.tick(
                black_box(tick),
                &mut activations,
                &mut synapses,
                &observations,
            );
        })
    });

    // Clean up
    drop((ml, activations, synapses));
}

criterion_group!(benches, bench_ml_tick_n1000);
criterion_main!(benches);
