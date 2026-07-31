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

# Run one experiment: log stderr to a file, print a readable result line,
# and never abort the matrix on a single failure (failures are recorded).
run_exp() {
  local NAME="$1" LOG="$OUT/$NAME.log"
  shift
  # shellcheck disable=SC2086
  if $CARGO run --release --example simulate -- "$@" >"$LOG" 2>&1; then
    echo "  ✓ $NAME"
  else
    echo "  ✗ $NAME FAILED (exit $?) — see $LOG"
    tail -5 "$LOG" | sed 's/^/    | /' || true
  fi
}

# ── E1: Convergence scaling ──────────────────────────────────────────
# 10 / 25 / 50 / 100 / 500 nodes, 3 fixed seeds each, 60s duration
run_e1() {
  echo "── E1: convergence scaling ──"
  local NODES="10 25 50 100 500"
  [ "$PROFILE" = "quick" ] && NODES="10 50 100"
  for N in $NODES; do
    for S in 42 1337 9001; do
      echo "  E1 nodes=$N seed=$S"
      run_exp "E1_nodes${N}_seed${S}" \
        --nodes "$N" --duration 60 --seed "$S" --paper-mode \
        --output-dir "$OUT/E1_nodes${N}_seed${S}"
    done
  done
}

# ── E4: Packet loss (deterministic in-sim impairment) ────────────────
run_e4() {
  echo "── E4: packet loss ──"
  for L in 0.02 0.05 0.10; do
    echo "  E4 loss=$L (100 nodes, 90s)"
    run_exp "E4_loss_${L}" \
      --nodes 100 --duration 90 --seed 42 --paper-mode \
      --packet-loss "$L" \
      --output-dir "$OUT/E4_loss_${L}"
  done
}

# ── E9: Baseline ablations (feature toggles, 50 nodes) ───────────────
run_e9() {
  echo "── E9: baseline ablations ──"
  run_e9_one "control"            ""
  run_e9_one "no-trust"           "--disable-trust"
  run_e9_one "no-aging"           "--disable-aging"
  run_e9_one "no-apoptosis"       "--disable-apoptosis"
  run_e9_one "no-neurogenesis"    "--disable-neurogenesis"
  run_e9_one "random-discovery"   "--random-discovery"
  run_e9_one "static-topology"    "--static-topology"
}

run_e9_one() {
  local NAME="$1" FLAGS="$2"
  echo "  E9 ${NAME} (50 nodes, 60s)"
  # shellcheck disable=SC2086
  run_exp "E9_${NAME}" \
    --nodes 50 --duration 60 --seed 42 --paper-mode \
    $FLAGS \
    --output-dir "$OUT/E9_${NAME}"
}

# ── E2: Node churn (death at t=30s; measure recovery) ───────────────
run_e2() {
  echo "── E2: node churn ──"
  for P in 0.10 0.20 0.50; do
    echo "  E2 churn=$P (100 nodes, death at 30s)"
    run_exp "E2_churn_${P}" \
      --nodes 100 --duration 90 --seed 42 --paper-mode \
      --failure-mode node-death --failure-at 30 --failure-percent "$P" \
      --output-dir "$OUT/E2_churn_${P}"
  done
}

# ── E5: Malicious peer injection ────────────────────────────────────
run_e5() {
  echo "── E5: malicious peer ──"
  run_exp "E5_malicious" \
    --nodes 100 --duration 90 --seed 42 --paper-mode \
    --failure-mode malicious --failure-at 20 \
    --output-dir "$OUT/E5_malicious"
}

# ── E6: Network partition + recovery ────────────────────────────────
run_e6() {
  echo "── E6: network partition ──"
  run_exp "E6_partition" \
    --nodes 100 --duration 90 --seed 42 --paper-mode \
    --failure-mode partition --failure-at 20 \
    --output-dir "$OUT/E6_partition"
}

# ── Run all ─────────────────────────────────────────────────────────
run_e1
if [ "$PROFILE" = "full" ]; then
  run_e2
  run_e4
  run_e5
  run_e6
  run_e9
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
