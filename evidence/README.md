# Evidence Pipeline — Reproducibility Package

The `evidence/` directory turns the Neuron Wire grant claim into something a
reviewer can verify on one machine in under an hour.

## What is validated here

1. **Deterministic simulation** — `run_matrix.sh` runs the E1–E9 experiment
   matrix with fixed seeds (42, 1337, 9001) via `examples/simulate.rs` in
   `--paper-mode`. Every run writes frozen config (`experiment.toml`), metadata
   (`metadata.json`), and raw CSVs.

2. **Local multi-process networking** — `localhost_cluster.sh` launches N real
   `node` processes on localhost, each with its own UDP port, health port,
   identity, storage, and log. Real sockets, real OS processes — the actual
   networking engine, not the simulator.

3. **Network emulation** — `emulate_network.sh` (Linux, root) drives the
   localhost cluster through `tc netem` impairments: normal (20ms/0%),
   mobile (80ms/2%), weak (150ms/5%), severe (300ms/10%).

## Quick start

```bash
# 1. Deterministic matrix (full: E1+E2+E5+E6, ~15 sim runs)
./evidence/run_matrix.sh

# 2. Local cluster: 2 / 5 / 10 / 25 real nodes
./evidence/localhost_cluster.sh 2 30
./evidence/localhost_cluster.sh 25 30

# 3. Network emulation (Linux only, needs root)
sudo ./evidence/emulate_network.sh 5 45
```

## Output layout

```
results/
├── evidence/            # simulation matrix (aggregated by aggregate.py)
│   ├── E1_nodes100_seed42/{experiment.toml, metadata.json, *.csv, raw/}
│   └── evidence_master.csv
└── localhost_cluster_5/ # node logs, health checks, metrics samples
```

## Roadmap (funded-phase M4)

- E3/E4 (latency/loss curves inside the simulator) — requires adding
  packet-loss/latency models to the in-process transport.
- E7/E8 (gradient aging, trust dynamics) — CLI exposure of aging params and
  trust telemetry.
- E9 (baselines) — simulator feature toggles: no-trust, no-apoptosis,
  no-neurogenesis, plain-Kademlia, random-discovery, static-topology, gossip
  without aging. Each toggle is a `--disable-<feature>` flag on `simulate`.
- Partition/churn/attack emulation at the process level — Toxiproxy or
  iptables-based splits wired into `emulate_network.sh`.

## CI wiring

A dedicated workflow (`evidence.yml`) runs `run_matrix.sh` (quick profile) +
`localhost_cluster.sh 5` on every push to `grant-prep`, and archives
`results/` as an artifact so every commit has a fresh evidence trail.
