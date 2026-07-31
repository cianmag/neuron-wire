# Neuron Wire Dashboard

The **neuron-wire** node ships a self-contained production dashboard that
runs on every node — no external dependencies, no Vercel, no Go.

## Usage

Start any NWP node with the health endpoint enabled:

```bash
NWP_HEALTH_BIND=0.0.0.0:9100 cargo run --bin node
```

Then open **http://<node-ip>:9100/** in your browser.

## What you see

- **Tick rate** — actual engine Hz (live)
- **Peers** — nodes in the DHT routing table
- **Packets in/out** — cumulative traffic counters
- **Idle ratio** — CPU utilisation gauge (green/yellow/red)
- **Retransmissions** — reliability-layer health
- **Live event log** — latest actions streamed

All data comes from `GET /status` (JSON) polled every 3s.

## External monitoring

The node exposes these additional endpoints:

| Endpoint | Format | Use |
|----------|--------|-----|
| `/health` | `{"status":"ok"}` | Liveness probe |
| `/status` | Full JSON dump | Dashboard data source |
| `/metrics` | Prometheus text | Scrape by Grafana |

## Vercel alternative (optional)

If you want a hosted dashboard separate from your VPS:

```bash
# 1. Set env var pointing to your node
vercel env add NWP_NODE_URL https://your-node:9100

# 2. Deploy
cd dashboard
vercel deploy --prod
```

The Vercel function in `api/node.go` proxies `/health`, `/status`, `/metrics`
to the NWP node and serves the dashboard UI.
