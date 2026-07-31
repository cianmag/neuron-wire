# Quick Start

## Build from Source

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build --release
```

## Run a Node

```bash
# Start with default config
./target/release/nwp-node

# Or with custom config
./target/release/nwp-node --config node.toml
```

## Configuration

Create `node.toml`:

```toml
[node]
name = "my-node"
bind_addr = "0.0.0.0:9000"
max_peers = 500
per_ip_max_peers = 10

[dht]
bootstrap = ["192.168.1.1:9000"]
```

## Connect to Peers

The node automatically:
1. Discovers peers via DNS seeds
2. Joins the DHT network
3. Begins heartbeat exchanges
4. Starts trust scoring

## Monitor

- Health: `http://localhost:9100/health`
- Metrics: `http://localhost:9100/metrics` (Prometheus)
- Dashboard: `http://localhost:9100/dashboard`

## Run Tests

```bash
cargo test --lib                    # Unit tests
cargo test --test integration       # Integration tests
cargo test --test security_integration  # Security tests
cargo test --test proptest          # Property-based tests
```
