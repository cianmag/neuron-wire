# Deployment

## Docker Compose

```bash
docker-compose up -d
```

Services: nwp-node, Prometheus, Grafana

## Systemd

```ini
[Unit]
Description=Neuron Wire Protocol Node
After=network.target

[Service]
ExecStart=/usr/local/bin/nwp-node --config /etc/nwp/node.toml
Restart=always
User=nwp
NoNewPrivileges=yes
ProtectSystem=strict
ReadWritePaths=/var/lib/nwp

[Install]
WantedBy=multi-user.target
```

## Monitoring

| Endpoint | Purpose |
|----------|---------|
| `/health` | Liveness probe |
| `/status` | Node status JSON |
| `/metrics` | Prometheus metrics (28+) |
| `/dashboard` | Live observability dashboard |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| NWP_BIND | 0.0.0.0:9000 | UDP bind address |
| NWP_MAX_PEERS | 500 | Max tracked peers |
| NWP_PER_IP_PEERS | 10 | Max per-IP connections |
| NWP_SECURITY | true | Enable packet signing |
| NWP_ENCRYPT | false | Enable payload encryption |
| NWP_STUN | false | Enable NAT traversal |
