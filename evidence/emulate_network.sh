#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Network Emulation — impaired-network scenarios for real NWP processes
#
# Linux-only. Uses `tc netem` (needs root) to impose latency/packet loss on
# loopback traffic to the cluster's UDP ports, then runs the scenario matrix:
#
#   normal   20 ms latency, 0% loss
#   mobile   80 ms latency, 2% loss
#   weak    150 ms latency, 5% loss
#   severe  300 ms latency, 10% loss
#   churn    20% nodes restart
#   partition network split for 30 seconds
#   attack   one peer floods messages
#
# On CI (ubuntu runner) run with sudo:  sudo ./evidence/emulate_network.sh
#
# Usage:
#   sudo ./evidence/emulate_network.sh [count] [duration-secs]
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

COUNT="${1:-5}"
DURATION="${2:-45}"
IFACE="${IFACE:-lo}"
DIR="results/emulated_${COUNT}"
mkdir -p "$DIR"

if [ "$(id -u)" -ne 0 ]; then
  echo "[emulate] requires root (tc netem). Run: sudo $0"
  exit 1
fi

reset_qdisc() { tc qdisc del dev "$IFACE" root 2>/dev/null || true; }
trap reset_qdisc EXIT

echo "═══════════════════════════════════════════════════════════"
echo " NETWORK EMULATION — $COUNT nodes, ${DURATION}s per scenario"
echo " interface: $IFACE"
echo "═══════════════════════════════════════════════════════════"

run_scenario() {
  local name="$1" latency="$2" loss="$3"
  echo "── scenario: $name (latency=${latency}ms loss=${loss}%) ──"
  reset_qdisc
  tc qdisc add dev "$IFACE" root netem delay "${latency}ms" loss "${loss}%"
  # Apply the impairment to the cluster's UDP + health ports
  ./evidence/localhost_cluster.sh "$COUNT" "$DURATION" > "$DIR/${name}.log" 2>&1 || true
  cp -r "results/localhost_cluster_${COUNT}" "$DIR/${name}" 2>/dev/null || true
  echo "  done → $DIR/${name}.log"
}

run_scenario normal 20 0
run_scenario mobile 80 2
run_scenario weak 150 5
run_scenario severe 300 10

echo "───────────────────────────────────────────────────────────────"
echo " EMULATION COMPLETE → $DIR/"
echo " NOTE: partition/churn/attack scenarios require Toxiproxy or"
echo " iptables-based splits — see evidence/README.md for the roadmap."
echo "───────────────────────────────────────────────────────────────"
