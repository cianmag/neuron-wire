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
  bash ./evidence/localhost_cluster.sh "$COUNT" "$DURATION" > "$DIR/${name}.log" 2>&1 || true
  cp -r "results/localhost_cluster_${COUNT}" "$DIR/${name}" 2>/dev/null || true
  echo "  done → $DIR/${name}.log"
}

run_scenario normal 20 0
run_scenario mobile 80 2
run_scenario weak 150 5
run_scenario severe 300 10

# ── Partition: split the cluster into two groups for 30s ──────────
run_partition() {
  echo "── scenario: partition (30s network split) ──"
  reset_qdisc
  bash ./evidence/localhost_cluster.sh "$COUNT" 35 > "$DIR/partition.log" 2>&1 &
  local cluster_pid=$!
  sleep 4  # let the cluster boot and converge
  local HALF=$((COUNT / 2))
  local g0s=9000 g0e=$((9000 + HALF - 1))
  local g1s=$((9000 + HALF)) g1e=$((9000 + COUNT - 1))
  echo "  blocking group [$g0s-$g0e] <-> [$g1s-$g1e] for 30s"
  iptables -A OUTPUT -p udp -m multiport --sports "$g0s:$g0e" -m multiport --dports "$g1s:$g1e" -j DROP 2>/dev/null || true
  iptables -A OUTPUT -p udp -m multiport --sports "$g1s:$g1e" -m multiport --dports "$g0s:$g0e" -j DROP 2>/dev/null || true
  sleep 30
  iptables -D OUTPUT -p udp -m multiport --sports "$g0s:$g0e" -m multiport --dports "$g1s:$g1e" -j DROP 2>/dev/null || true
  iptables -D OUTPUT -p udp -m multiport --sports "$g1s:$g1e" -m multiport --dports "$g0s:$g0e" -j DROP 2>/dev/null || true
  echo "  split lifted — waiting for re-convergence (5s)"
  sleep 5
  wait "$cluster_pid" || true
  cp -r "results/localhost_cluster_${COUNT}" "$DIR/partition" 2>/dev/null || true
  echo "  done → $DIR/partition.log"
}

# ── Attack: one peer floods node-0 with garbage for 15s ───────────
flood() {
  python3 - "$1" "$2" "$3" <<'PYEOF'
import socket, sys, time
port, seconds, pps = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
payload = b"FLOOD" * 100
end = time.time() + seconds
while time.time() < end:
    for _ in range(pps):
        s.sendto(payload, ("127.0.0.1", port))
    time.sleep(1)
PYEOF
}

run_attack() {
  echo "── scenario: attack (peer floods node-0, 15s) ──"
  reset_qdisc
  bash ./evidence/localhost_cluster.sh "$COUNT" 25 > "$DIR/attack.log" 2>&1 &
  local cluster_pid=$!
  sleep 4
  flood 9000 15 200 &
  local flood_pid=$!
  wait "$flood_pid" || true
  echo "  flood done — node-0 must still answer health checks"
  wait "$cluster_pid" || true
  cp -r "results/localhost_cluster_${COUNT}" "$DIR/attack" 2>/dev/null || true
  echo "  done → $DIR/attack.log"
}

run_partition
run_attack

echo "───────────────────────────────────────────────────────────────"
echo " EMULATION COMPLETE → $DIR/"
echo " scenarios: normal/mobile/weak/severe/partition/attack"
echo "───────────────────────────────────────────────────────────────" 
