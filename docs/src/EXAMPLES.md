# Examples

Neuron-wire ships with several example programs to help you understand the protocol and run experiments.

> **Full documentation:** [`examples/README.md`](https://github.com/cianmag/neuron-wire/blob/master/examples/README.md)

## Available Examples

| Example | Description | Command |
|---------|-------------|---------|
| `simulate` | Multi-trial simulation benchmark | `cargo run --example simulate -- --nodes 5 --duration 25` |
| `zero_copy_demo` | FlatBuffer protocol pipeline | `cargo run --example zero_copy_demo` |
| `dashboard` | Live observability dashboard | `cargo run --example dashboard` |
| `chat` | Interactive P2P messaging | `cargo run --example chat -- --port 9000 --name alice` |
| `tune` | Hyperparameter sweep | `cargo run --release --example tune` |

## Quick Start

```bash
# Clone and run the dashboard
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo run --example dashboard
# Open http://localhost:9090
```

## Running All Examples

```bash
cargo check --examples
```

See the [examples directory](https://github.com/cianmag/neuron-wire/tree/master/examples) for source code.
