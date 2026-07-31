# Releasing neuron-wire

This document describes the release process for neuron-wire.

## Overview

Releases follow [Semantic Versioning](https://semver.org/):

- **Major** (X.0.0): Breaking API changes
- **Minor** (0.X.0): New features, backwards-compatible
- **Patch** (0.0.X): Bug fixes, backwards-compatible

All releases are tagged and published as GitHub Releases with pre-built binaries for Linux (x64, ARM64), macOS (x64), and Windows (x64).

## Prerequisites

- `cargo` (Rust 1.87+)
- Git with push access to `main`
- GitHub CLI (`gh`) — optional, for creating releases manually

## Release Steps

### 1. Update the version in Cargo.toml

```bash
# Example: bumping to 0.4.0
cargo edit --version 0.4.0
```

Or manually edit `Cargo.toml`:

```toml
[package]
version = "0.4.0"
```

### 2. Update CHANGELOG.md

Move items from `[Unreleased]` into a new version section with today's date:

```markdown
## [0.4.0] - 2026-07-25

### Added
- Feature description

### Fixed
- Bug fix description
```

Update the comparison links at the bottom:

```markdown
[Unreleased]: https://github.com/cianmag/neuron-wire/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/cianmag/neuron-wire/compare/v0.3.0...v0.4.0
```

### 3. Verify the build

```bash
cargo build --release --locked
cargo test
cargo clippy -- -D warnings
```

### 4. Commit and tag

```bash
git add -A
git commit -m "release: v0.4.0"
git tag -a v0.4.0 -m "Release v0.4.0"
git push origin main --tags
```

### 5. GitHub Actions handles the rest

Pushing the `v*` tag triggers the release workflow:

1. Builds release binaries for all 4 targets (linux-x64, linux-arm64, macos-x64, windows-x64)
2. Extracts the changelog section for this version
3. Creates a GitHub Release with binaries attached

Monitor the release at:
```
https://github.com/cianmag/neuron-wire/releases
```

## Manual Release (Fallback)

If GitHub Actions is unavailable:

```bash
# Build for current platform
cargo build --release --locked

# Create a release manually
gh release create v0.4.0 \
  --title "v0.4.0" \
  --notes-file release_notes.md \
  target/release/neuron-wire
```

## Cross-Compilation

For building ARM64 on x64 hosts:

```bash
rustup target add aarch64-unknown-linux-gnu
sudo apt install gcc-aarch64-linux-gnu
cargo build --release --locked --target aarch64-unknown-linux-gnu
```

For macOS from Linux (requires Docker + osxcross):

```bash
# Typically handled by CI — local cross-compilation is not supported
```

## Docker Images

After a release, build and push the Docker image:

```bash
docker build -t neuron-wire:0.4.0 .
docker tag neuron-wire:0.4.0 neuron-wire:latest
docker push neuron-wire:0.4.0
docker push neuron-wire:latest
```

Or let Docker Compose handle it:

```bash
docker compose build
docker compose up -d
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `Cargo.lock` out of date | Run `cargo update` before committing |
| Build fails on ARM64 | Ensure `gcc-aarch64-linux-gnu` is installed |
| Tag already exists | Delete with `git tag -d v0.4.0 && git push origin :refs/tags/v0.4.0` |
| Release workflow skipped | Check that the tag matches `v*` pattern exactly |
| Binary not found in release | Check the `target/` directory for the correct triple |

## Version History

See [CHANGELOG.md](CHANGELOG.md) for the full version history.
