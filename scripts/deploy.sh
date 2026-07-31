#!/usr/bin/env bash
# ─── Deploy neuron-wire node binary to a VPS ──────────────────────
# Usage:
#   export VPS_USER=root VPS_HOST=1.2.3.4
#   ./scripts/deploy.sh
#
# Prerequisites:
#   - Rust toolchain (for local binary build)
#   - rsync or scp available locally
#   - rsync or scp/sshd running on VPS
#
# This script:
#   1. Builds the node binary in release mode with musl target
#   2. Copies the binary + config template to the VPS
#   3. Installs the binary as a systemd service
#   4. Starts the service

set -euo pipefail
cd "$(dirname "$0")/.."

# ── Configuration ──────────────────────────────────────────────────
VPS_USER="${VPS_USER:-root}"
VPS_HOST="${VPS_HOST:?VPS_HOST not set (e.g. export VPS_HOST=1.2.3.4)}"
VPS_PORT="${VPS_PORT:-22}"
REMOTE_DIR="${REMOTE_DIR:-/opt/nwp}"
BINARY="${BINARY:-target/x86_64-unknown-linux-musl/release/node}"
SERVICE_NAME="nwp-node"

# ── Step 1: Build ─────────────────────────────────────────────────
echo "→ Building release binary (musl target)..."
cargo build --release --bin node --target x86_64-unknown-linux-musl

# ── Step 2: Copy to VPS ──────────────────────────────────────────
echo "→ Copying binary + config to ${VPS_USER}@${VPS_HOST}:${REMOTE_DIR}..."
ssh "${VPS_USER}@${VPS_HOST}" -p "${VPS_PORT}" \
    "mkdir -p ${REMOTE_DIR}"

rsync -avz -e "ssh -p ${VPS_PORT}" \
    "${BINARY}" \
    "${VPS_USER}@${VPS_HOST}:${REMOTE_DIR}/nwp-node"

rsync -avz -e "ssh -p ${VPS_PORT}" \
    "node-config.example.toml" \
    "${VPS_USER}@${VPS_HOST}:${REMOTE_DIR}/node-config.toml"

# ── Step 3: Install systemd service ──────────────────────────────
echo "→ Installing systemd service..."
ssh "${VPS_USER}@${VPS_HOST}" -p "${VPS_PORT}" bash -s <<'EOSCRIPT'
set -euo pipefail

REMOTE_DIR="/opt/nwp"
SERVICE_NAME="nwp-node"

cat > /etc/systemd/system/${SERVICE_NAME}.service <<'EOSERVICE'
[Unit]
Description=Neuron Wire Protocol Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/nwp
ExecStart=/opt/nwp/nwp-node --config /opt/nwp/node-config.toml --identity /opt/nwp/identity.key
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

# Security hardening
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=full
ProtectHome=yes
CapabilityBoundingSet=~CAP_NET_RAW
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
EOSERVICE

systemctl daemon-reload
systemctl enable ${SERVICE_NAME}
systemctl restart ${SERVICE_NAME}

echo "→ Service status:"
systemctl status ${SERVICE_NAME} --no-pager

# Show recent logs
echo "→ Recent logs:"
journalctl -u ${SERVICE_NAME} -n 20 --no-pager
EOSCRIPT

echo ""
echo "✅ Deployment complete!"
echo "   SSH into your VPS and check:"
echo "     systemctl status nwp-node"
echo "     journalctl -u nwp-node -f"
