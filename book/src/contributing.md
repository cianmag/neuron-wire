# Contributing

## Development Setup

```bash
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
cargo build
cargo test
```

## Testing

```bash
cargo test --lib                    # Unit tests (273+)
cargo test --test integration       # Integration tests (17)
cargo test --test security_integration  # Security tests (7)
cargo test --test stress            # Stress tests (9)
cargo test --test proptest          # Property-based tests (20)
cargo test --test profiling         # Performance profiles (6)
```

## Code Quality

- `#![warn(missing_docs)]` — all public items must be documented
- `// SAFETY:` comments on all unsafe blocks
- Property-based tests for all crypto operations
- No `unwrap()` in production code (use `if let` or `?`)

## CI Pipeline

9 GitHub Actions jobs:
- Build + test (stable, nightly)
- Clippy lint + rustfmt
- Benchmark regression (>5% = fail)
- Security audit (cargo-audit, cargo-deny)
- Fuzzing (cargo-fuzz)
- Documentation (cargo doc)
