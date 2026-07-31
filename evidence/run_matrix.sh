#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# NWP Evidence Matrix Runner — reproducible grant experiments
#
# Runs the E1–E9 experiment matrix with FIXED seeds (paper mode) and writes
# raw CSV/TOML output to results/evidence/. Every run is deterministic.
#
# Usage:
#   ./evidence/run_matrix.sh [output-dir] [profile]
#     output-dir  default: results/evidence
#     profile     "full" (default) | "quick" (E1 only, 3 node counts)
#
# Reproduce everything:
#   git clone https://github.com/cianmag/neuron-wire
#   cd neuron-wire
#   ./evidence/run_matrix.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

OUT="${1:-results/evidence}"
PROFILE="${2:-full}"
CARGO="cargo"
mkdir -p "$OUT"

echo "═══════════════════════════════════════════════════════════"
echo " NWP EVIDENCE MATRIX — profile=$PROFILE → $OUT"
echo "═══════════════════════════════════════════════════════════"

# ── E1: Convergence scaling ──────────────────────────────────────────
# 10 / 25 / 50 / 100 / 500 nodes, 3 fixed seeds each, 60s duration
run_e1() {
  echo "── E1: convergence scaling ──"
  local NODES="10 25 50 100 500"
  [ "$PROFILE" = "quick" ] && NODES="10 50 100"
  for N in $NODES; do
    for S in 42 1337 9001; do
      echo "  E1 nodes=$N seed=$S"
      $CARGO run --release --example simulate -- \
        --nodes "$N" --duration 60 --seed "$S" --paper-mode \
        --output-dir "$OUT/E1_nodes${N}_seed${S}" 2>/dev/null
    done
  done
}

# ── E2: Node churn (death at t=30s; measure recovery) ───────────────
run_e2() {
  echo "── E2: node churn ──"
  for P in 0.10 0.20 0.50; do
    echo "  E2 churn=$P (100 nodes, death at 30s)"
    $CARGO run --release --example simulate -- \
      --nodes 100 --duration 90 --seed 42 --paper-mode \
      --failure-mode node-death --failure-at 30 --failure-percent "$P" \
      --output-dir "$OUT/E2_churn_${P}" 2>/dev/null
  done
}

# ── E5: Malicious peer injection ────────────────────────────────────
run_e5() {
  echo "── E5: malicious peer ──"
  $CARGO run --release --example simulate -- \
    --nodes 100 --duration 90 --seed 42 --paper-mode \
    --failure-mode malicious --failure-at 20 \
    --output-dir "$OUT/E5_malicious" 2>/dev/null
}

# ── E6: Network partition + recovery ────────────────────────────────
run_e6() {
  echo "── E6: network partition ──"
  $CARGO run --release --example simulate -- \
    --nodes 100 --duration 90 --seed 42 --paper-mode \
    --failure-mode partition --failure-at 20 \
    --output-dir "$OUT/E6_partition" 2>/dev/null
}

# ── Run all ─────────────────────────────────────────────────────────
run_e1
if [ "$PROFILE" = "full" ]; then
  run_e2
  run_e5
  run_e6
fi

# ── Aggregate into a master evidence table ──────────────────────────
echo "── Aggregating ──"
if command -v python3 >/dev/null 2>&1; then
  python3 evidence/aggregate.py "$OUT"
else
  echo "[matrix] python3 not found — skip aggregation"
fi

echo "───────────────────────────────────────────────────────────────"
echo " DONE → $OUT  (commit this directory as the evidence artifact)"
echo "───────────────────────────────────────────────────────────────"
