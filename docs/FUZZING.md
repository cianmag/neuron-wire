# Fuzz Testing

This directory contains [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets
for the security-critical parsing code in neuron-wire.

## Prerequisites

```bash
# Install cargo-fuzz (requires nightly Rust)
rustup toolchain install nightly
rustup component add rustfmt --toolchain nightly
cargo install cargo-fuzz
```

## Fuzz Targets

| Target | Module Under Test | What It Fuzzes |
|--------|-------------------|----------------|
| `fuzz_header_parse` | `header::parse_frame()` | Frame parsing: magic validation, version check, CRC verification, body length bounds |
| `fuzz_transport_header` | `transport::TransportHeader::from_bytes()` | Unsafe zero-copy header deserialization, field extraction, roundtrip |
| `fuzz_trust_deserialize` | Trust binary format (from `trust.rs`) | Peer count parsing, entity ID extraction, f32 score validation, u64 event count, truncated input |
| `fuzz_flat_body` | `flat::BodyReader` | Zero-copy field accessors, offset calculation, string/bytes reading, `from_utf8_unchecked` |

## Running

```bash
# Run the header parsing fuzzer (infinite until Ctrl+C)
cargo +nightly fuzz run fuzz_header_parse

# Run with a time limit (seconds)
cargo +nightly fuzz run fuzz_header_parse -- -max_total_time=300

# Run a specific target
cargo +nightly fuzz run fuzz_transport_header
cargo +nightly fuzz run fuzz_trust_deserialize
cargo +nightly fuzz run fuzz_flat_body

# Run all targets sequentially
cargo +nightly fuzz run fuzz_header_parse -- -max_total_time=120
cargo +nightly fuzz run fuzz_transport_header -- -max_total_time=120
cargo +nightly fuzz run fuzz_trust_deserialize -- -max_total_time=120
cargo +nightly fuzz run fuzz_flat_body -- -max_total_time=120

# List all fuzz targets
cargo +nightly fuzz list
```

## Configuration

Fuzz settings are in `fuzz/fuzz.toml`. Key parameters:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_total_time` | `0` (unlimited) | Total fuzzing time in seconds |
| `max_len` | `4096` | Max bytes per fuzz input |
| `timeout` | `10` | Per-execution timeout in seconds |
| `rss_limit_mb` | `256` | Memory limit per execution |

## Coverage

```bash
# Generate coverage report for a specific target
cargo +nightly fuzz coverage fuzz_header_parse

# Coverage report will be in fuzz/coverage/<target>/
```

## Corpus

- **`corpus/`** — Interesting inputs discovered by the fuzzer. Commit these
  to reproduce and regression-test edge cases.
- **`artifacts/`** — Crashing inputs. If the fuzzer finds a crash, the input
  is saved here. Investigate and add as a regression test in `src/*/tests.rs`.

## Architecture

Each fuzz target:

1. Takes an arbitrary `&[u8]` slice from `libfuzzer-sys`
2. Wraps the parser call in `std::panic::catch_unwind` to prevent panics
   from killing the fuzzer process
3. Calls the parser function and discards the result
4. Returns `Ok(())` — the fuzzer only cares about panics and memory errors

The trust deserialization target reimplements the binary format parser from
`trust.rs::load_from_file` directly on a byte slice (no temp file needed),
making it suitable for fuzzing without filesystem side effects.

## Why Fuzz These?

These modules handle **untrusted network input**:

- **`header::parse_frame()`** — First thing called on every incoming NWP message.
  A bug here could allow remote code execution via crafted packets.
- **`transport::TransportHeader::from_bytes()`** — `unsafe` zero-copy cast from
  raw UDP datagram bytes. Misaligned or malicious input could cause UB.
- **Trust binary format** — Loaded from disk, but disk content may be attacker-controlled
  in a compromised node scenario.
- **`flat::BodyReader`** — Zero-copy body parsing with `from_utf8_unchecked`.
  Malformed offsets could cause out-of-bounds reads.
