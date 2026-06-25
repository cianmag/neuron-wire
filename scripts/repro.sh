#!/usr/bin/env bash
# Reproduce DHT convergence benchmark — one command, zero configuration.
# Run from the repository root.
#
# Usage:
#   bash scripts/repro.sh
#
# Does: builds, runs 3-node paper-mode, validates against known-good.

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"

echo "═══════════════════════════════════════════════════"
echo "  Neuron-Wire DHT — Reproduction Script"
echo "═══════════════════════════════════════════════════"
echo ""

# 1. Build
echo "▸ Building..."
cargo build --quiet --example simulate 2>/dev/null || cargo build --example simulate
echo "  ✓ Build complete"
echo ""

# 2. Run paper-mode benchmark (3 nodes, 10s)
OUTDIR="results/reproduction-$(date +%s)"

echo "▸ Running paper-mode benchmark (3 nodes, 10s)..."
cargo run --example simulate -- --paper-mode --nodes 3 --duration 10 \
  --output-dir "${OUTDIR}" 2>&1 | tail -12
echo ""

# 3. Validate
echo "▸ Validating against known-good..."
if python scripts/validate_repro.py "${OUTDIR}" --known-good known-good/; then
  echo ""
  echo "═══════════════════════════════════════════════════"
  echo "  ✅ REPRODUCTION SUCCESSFUL"
  echo "  Results match known-good reference."
  echo "═══════════════════════════════════════════════════"
else
  echo ""
  echo "═══════════════════════════════════════════════════"
  echo "  ❌ REPRODUCTION MISMATCH"
  echo "  See diff above. Results differ from known-good."
  echo "  This may be a platform-specific issue."
  echo "═══════════════════════════════════════════════════"
  exit 1
fi
