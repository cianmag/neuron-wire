#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Local Multi-Process Test — real NWP node processes on localhost
#
# Launches N real `node` binaries, each with its own:
#   - UDP port        (9000+i)
#   - health port     (9100+i)
#   - config file     (tmpdir/node-<i>.toml)
#   - identity file   (tmpdir/identity-<i>.key)
#   - peer cache      (tmpdir/peers-<i>.bin)
#   - trust cache     (tmpdir/trust-<i>.bin)
#   - log output      (tmpdir/node-<i>.log)
#
# This proves the actual networking engine works outside the simulator:
# real sockets, real OS processes, real clocks. (These are local
# multi-process nodes — NOT deployed nodes.)
#
# Usage:
#   ./evidence/localhost_cluster.sh [count] [duration-secs] [binary]
#   ./evidence/localhost_cluster.sh 25 60 target/release/node
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

COUNT="${1:-5}"
DURATION="${2:-30}"
BIN="${3:-target/release/node}"
DIR="results/localhost_cluster_${COUNT}"
mkdir -p "$DIR"

echo "═══════════════════════════════════════════════════════════"
echo " LOCAL MULTI-PROCESS TEST — $COUNT real nodes, ${DURATION}s"
echo " binary: $BIN"
echo "═══════════════════════════════════════════════════════════"

if [ ! -x "$BIN" ]; then
  echo "[cluster] building $BIN (release)..."
  CARGO_BIN="${CARGO:-cargo}"
  "$CARGO_BIN" build --release --bin node
fi

PIDS=()
cleanup() {
  echo "[cluster] shutting down ${#PIDS[@]} nodes..."
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  wait 2>/dev/null || true
  echo "[cluster] done. logs → $DIR/"
}
trap cleanup EXIT

# Node 0 starts first (bootstrap), others point at it
for i in $(seq 0 $((COUNT - 1))); do
  UDP_PORT=$((9000 + i))
  HEALTH_PORT=$((9100 + i))
  CFG="$DIR/node-${i}.toml"
  BOOTSTRAP=""
  if [ "$i" -gt 0 ]; then BOOTSTRAP="bootstrap_peers = [\"127.0.0.1:9000\"]"; fi
  cat > "$CFG" <<EOF
[node]
name = "local-${i}"
bind_addr = "127.0.0.1:${UDP_PORT}"
identity_file = "${DIR}/identity-${i}.key"
tick_interval_ms = 1
max_peers = 500
per_ip_max_peers = 50

[dht]
enabled = true
${BOOTSTRAP}
peer_cache_path = "${DIR}/peers-${i}.bin"
trust_cache_path = "${DIR}/trust-${i}.bin"

[stun]
enabled = false
EOF

  echo "[cluster] start node-$i udp=127.0.0.1:$UDP_PORT health=127.0.0.1:$HEALTH_PORT"
  NWP_HEALTH_BIND="127.0.0.1:${HEALTH_PORT}" \
    "$BIN" --config "$CFG" > "$DIR/node-${i}.log" 2>&1 &
  PIDS+=($!)
  sleep 0.3
done

echo "[cluster] all $COUNT nodes launched. waiting ${DURATION}s..."
sleep "$DURATION"

# ── Verify: count live health endpoints ─────────────────────────────
ALIVE=0
for i in $(seq 0 $((COUNT - 1))); do
  if curl -s -m 1 "http://127.0.0.1:$((9100 + i))/health" >/dev/null 2>&1; then
    ALIVE=$((ALIVE + 1))
  else
    echo "[cluster] ⚠ node-$i health endpoint NOT responding"
  fi
done
echo "[cluster] health check: ${ALIVE}/${COUNT} nodes alive"

# ── Gather observable stats ─────────────────────────────────────────
echo "[cluster] metrics sample (node-0):"
curl -s -m 2 "http://127.0.0.1:9100/metrics" 2>/dev/null | grep -E "^nwp_" | head -20 || echo "  (no metrics)"

echo "[cluster] result: $ALIVE/$COUNT nodes healthy after ${DURATION}s"
# Exit non-zero if any node died — CI can gate on this
[ "$ALIVE" -eq "$COUNT" ]
