# Fuzz Testing

This directory contains [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets.

## Prerequisites

```bash
# Install cargo-fuzz (requires nightly Rust)
rustup toolchain install nightly
cargo install cargo-fuzz
```

## Running

```bash
# Run the header parsing fuzzer (default: infinite, Ctrl+C to stop)
cargo +nightly fuzz run header_parse

# Run with a specific number of iterations
cargo +nightly fuzz run header_parse -- -max_total_time=60

# List all fuzz targets
cargo +nightly fuzz list
```

## Targets

| Target | Description |
|--------|-------------|
| `header_parse` | Feeds random bytes into `MessageHeader::from_bytes`. Should never panic. |

## Coverage

```bash
cargo +nightly fuzz coverage header_parse
```

## Corpus

The `corpus/` directory stores interesting inputs that trigger edge cases.
The `artifacts/` directory stores inputs that cause panics (if any).
