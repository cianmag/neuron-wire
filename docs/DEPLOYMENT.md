# Neuron Wire — Docker Deployment Guide

## Prerequisites

- Docker Engine 24+ and Docker Compose v2
- At least 1 GB free RAM (node + Prometheus + Grafana)
- Ports 9000/udp, 9100/tcp, 9090/tcp, 3000/tcp available on the host

## Quick Start

```bash
# 1. Clone and enter the repo
git clone https://github.com/cianmag/neuron-wire.git
cd neuron-wire

# 2. Create your environment file
cp .env.example .env
# Edit .env to set GRAFANA_PASSWORD and any port overrides

# 3. Build and start the full stack
docker compose up -d --build

# 4. Verify all services are healthy
docker compose ps

# 5. Watch logs
docker compose logs -f nwp-node
docker compose logs -f prometheus
docker compose logs -f grafana
```

## Accessing Services

| Service    | URL                              | Purpose                     |
|------------|----------------------------------|-----------------------------|
| NWP Dashboard | `http://localhost:9100`      | Live node dashboard (HTML)  |
| NWP Health | `http://localhost:9100/health`   | Liveness probe (JSON)       |
| NWP Metrics| `http://localhost:9100/metrics`  | Prometheus text format      |
| Prometheus | `http://localhost:9090`          | Metrics explorer & alerts   |
| Grafana    | `http://localhost:3000`          | Dashboards & visualization  |

Grafana login: `admin` / `nwp-admin` (change via `.env`).

## Scaling Nodes

Scale the NWP node horizontally — Prometheus auto-discovers new instances:

```bash
# Scale to 5 nodes
docker compose up -d --scale nwp-node=5

# Check running instances
docker compose ps nwp-node

# Scale back down
docker compose up -d --scale nwp-node=2
```

When scaling, each node gets a random suffix (e.g. `neuron-wire-nwp-node-1`).
They all share the same Docker Compose config and DHT bootstrap peers,
so they discover each other automatically on the `nwp-internal` network.

## Common Operations

```bash
# Stop everything (preserves volumes)
docker compose down

# Stop and remove all data (identity keys, Prometheus TSDB, Grafana DB)
docker compose down -v

# Rebuild after source changes
docker compose up -d --build nwp-node

# View Prometheus targets
curl -s http://localhost:9090/api/v1/targets | jq

# Reload Prometheus config without restart
curl -X POST http://localhost:9090/-/reload

# Check node health
curl http://localhost:9100/health
curl http://localhost:9100/status | jq
```

## Network Architecture

```
┌─────────────────────────────────────────────────┐
│                 Docker Host                      │
│                                                  │
│  ┌──────────┐   UDP/9000   ┌──────────┐         │
│  │ nwp-node │◄────────────►│ nwp-node │  ...    │
│  │   (1)    │  nwp-internal│   (N)    │         │
│  └────┬─────┘              └────┬─────┘         │
│       │ TCP/9100                │ TCP/9100       │
│       └──────────┬──────────────┘                │
│           nwp-monitoring network                 │
│                  │                               │
│           ┌──────┴──────┐                       │
│           │  Prometheus  │                       │
│           │   :9090      │                       │
│           └──────┬──────┘                       │
│                  │                               │
│           ┌──────┴──────┐                       │
│           │   Grafana    │                       │
│           │   :3000      │                       │
│           └─────────────┘                       │
└─────────────────────────────────────────────────┘
```

- **nwp-internal**: Node-to-node UDP traffic only. No monitoring services here.
- **nwp-monitoring**: HTTP traffic between nodes, Prometheus, and Grafana.

## Production Hardening

1. **Change defaults**: Set `GRAFANA_PASSWORD` and consider `encrypt_payloads = true`
2. **Firewall**: Only expose port 9000/udp to the public internet; keep 9100, 9090, 3000 on localhost or a private network
3. **TLS**: Place a reverse proxy (Caddy, nginx) in front of Grafana/Prometheus
4. **Backup**: Back up the `nwp-identity` volume — it contains the node's Ed25519 keypair
5. **Monitoring alerts**: Add `prometheus/alert_rules.yml` and connect Alertmanager

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `nwp-node` exits immediately | Check logs: `docker compose logs nwp-node`. Usually a bad config or missing identity key |
| Prometheus shows no targets | Ensure nodes are on the `nwp-monitoring` network and port 9100 is reachable |
| Grafana shows "No data" | Verify Prometheus datasource URL (`http://nwp-prometheus:9090`) and that `/metrics` returns data |
| High CPU on node | Reduce `tick_interval_ms` or increase `retransmit_interval_ms` in node config |
| Docker Compose scale fails | Only the first scaled instance gets the fixed container name; others use auto-generated names |
