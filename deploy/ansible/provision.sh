#!/usr/bin/env bash
# ─── Provision a VPS from scratch with Ansible ──────────────────
# Usage:
#   export VPS_IP=1.2.3.4
#   ./deploy/ansible/provision.sh
#
# This script:
#   1. Installs Ansible locally (if missing)
#   2. Creates inventory.yml from template with your VPS IP
#   3. Runs the Ansible playbook
#
# Prerequisites: Python 3 + pip on your dev machine.

set -euo pipefail
cd "$(dirname "$0")/../.."

VPS_IP="${VPS_IP:?VPS_IP not set (e.g. export VPS_IP=1.2.3.4)}"
VPS_USER="${VPS_USER:-root}"
INVENTORY="deploy/ansible/inventory.yml"
PLAYBOOK="deploy/ansible/playbook.yml"

# ── Step 1: Install Ansible if missing ──────────────────────────
if ! command -v ansible-playbook &>/dev/null; then
    echo "→ Installing Ansible..."
    pip install ansible --quiet
fi

# ── Step 2: Create inventory ────────────────────────────────────
echo "→ Creating inventory at ${INVENTORY}..."
cat > "${INVENTORY}" <<EOF
nwp_nodes:
  hosts:
    nwp-node:
      ansible_host: ${VPS_IP}
      ansible_user: ${VPS_USER}
  vars:
    nwp_peers: []
EOF

# ── Step 3: Run playbook ────────────────────────────────────────
echo "→ Running Ansible playbook against ${VPS_USER}@${VPS_IP}..."
ansible-playbook -i "${INVENTORY}" "${PLAYBOOK}" -v

echo ""
echo "✅ Provisioning complete!"
echo "   Check node status:"
echo "     ssh ${VPS_USER}@${VPS_IP} 'docker ps'"
echo "     curl http://${VPS_IP}:9100/health"
echo "     curl http://${VPS_IP}:9100/status"
