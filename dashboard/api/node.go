{{/*
Neuron Wire Dashboard — Vercel-ready Serverless Function
=========================================================

Reads from the NWP node's health endpoint and renders a real-time dashboard.
Configure with NWP_NODE_URL env var.

Deploy:
  vercel deploy --prod

Set environment variable:
  vercel env add NWP_NODE_URL https://your-node:9100
*/}}
package handler

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"
)

var nodeURL = os.Getenv("NWP_NODE_URL")

// HTML template for the dashboard
const dashboardHTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Neuron Wire — Live Dashboard</title>
<style>
  :root {
    --bg: #0a0a0f;
    --card: #12121a;
    --border: #1e1e30;
    --text: #c8c8d4;
    --muted: #68687a;
    --accent: #6c5ce7;
    --green: #00d68f;
    --yellow: #ffd43b;
    --red: #ff6b6b;
    --cyan: #64d2ff;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'SF Mono', 'Cascadia Code', 'JetBrains Mono', monospace;
    background: var(--bg);
    color: var(--text);
    padding: 24px;
    min-height: 100vh;
  }
  .header {
    display: flex; justify-content: space-between; align-items: center;
    margin-bottom: 24px; padding-bottom: 16px;
    border-bottom: 1px solid var(--border);
  }
  .header h1 {
    font-size: 20px; color: #fff; font-weight: 500;
    display: flex; align-items: center; gap: 12px;
  }
  .header .status-badge {
    font-size: 11px; padding: 3px 10px; border-radius: 99px;
    background: var(--green); color: #000; font-weight: 600;
  }
  .header .status-badge.error { background: var(--red); color: #fff; }
  .header .status-badge.warn { background: var(--yellow); color: #000; }
  .header .time { color: var(--muted); font-size: 12px; }
  .grid {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 12px; margin-bottom: 20px;
  }
  .card {
    background: var(--card); border: 1px solid var(--border);
    border-radius: 8px; padding: 16px;
  }
  .card .label { font-size: 11px; color: var(--muted); text-transform: uppercase;
    letter-spacing: 0.5px; margin-bottom: 6px; }
  .card .value { font-size: 28px; color: #fff; font-weight: 500; }
  .card .value.green { color: var(--green); }
  .card .value.yellow { color: var(--yellow); }
  .card .value.red { color: var(--red); }
  .card .value.cyan { color: var(--cyan); }
  .card .sub { font-size: 11px; color: var(--muted); margin-top: 2px; }
  .card-row { display: flex; gap: 8px; flex-wrap: wrap; }
  .card-row .mini { flex: 1; min-width: 100px; }
  .log { 
    background: var(--card); border: 1px solid var(--border);
    border-radius: 8px; padding: 16px;
    font-size: 12px; line-height: 1.6;
    max-height: 300px; overflow-y: auto;
  }
  .log .line { color: var(--muted); }
  .log .line .ts { color: #48485a; }
  .gauge-bar {
    height: 4px; background: var(--border); border-radius: 2px;
    margin-top: 8px; overflow: hidden;
  }
  .gauge-bar .fill {
    height: 100%; border-radius: 2px;
    transition: width 0.5s ease;
  }
  .gauge-bar .fill.green { background: var(--green); }
  .gauge-bar .fill.yellow { background: var(--yellow); }
  .gauge-bar .fill.red { background: var(--red); }
  .row { display: flex; gap: 16px; flex-wrap: wrap; }
  .row .card { flex: 1; }
  .footer { color: var(--muted); font-size: 11px; margin-top: 24px;
    text-align: center; padding-top: 16px; border-top: 1px solid var(--border); }
  .peers-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
  }
  .peer { 
    background: var(--bg); border: 1px solid var(--border);
    border-radius: 6px; padding: 10px; font-size: 12px;
  }
  .peer .addr { color: var(--cyan); font-size: 11px; }
  .peer .lat { color: var(--muted); font-size: 10px; }
  @media (max-width: 600px) {
    body { padding: 12px; }
    .grid { grid-template-columns: repeat(2, 1fr); }
    .card .value { font-size: 20px; }
  }
</style>
</head>
<body>
<div class="header">
  <h1>🧠 Neuron Wire <span id="status-badge" class="status-badge">loading</span></h1>
  <span class="time" id="update-time">—</span>
</div>

<div class="grid" id="stat-cards">
  <div class="card"><div class="label">Tick Rate</div><div class="value cyan" id="tick-rate">—</div><div class="sub">Hz</div></div>
  <div class="card"><div class="label">Total Ticks</div><div class="value" id="total-ticks">—</div></div>
  <div class="card"><div class="label">Peers</div><div class="value green" id="peer-count">—</div><div class="sub">known nodes</div></div>
  <div class="card"><div class="label">Packets In</div><div class="value" id="packets-recv">—</div><div class="sub">received</div></div>
  <div class="card"><div class="label">Packets Out</div><div class="value" id="packets-sent">—</div><div class="sub">sent</div></div>
  <div class="card"><div class="label">Idle Ratio</div><div class="value" id="idle-ratio">—</div><div class="sub">% idle</div><div class="gauge-bar"><div class="fill green" id="idle-gauge" style="width:0%"></div></div></div>
  <div class="card"><div class="label">Retransmissions</div><div class="value yellow" id="retransmissions">—</div></div>
  <div class="card"><div class="label">Reliable Queue</div><div class="value" id="reliable-q">—</div></div>
</div>

<div class="row">
  <div class="card" style="flex:2">
    <div class="label">Traffic</div>
    <div class="card-row" style="margin-top:8px">
      <div class="mini"><div class="label">Bytes In</div><div class="value cyan" id="bytes-recv" style="font-size:18px">—</div></div>
      <div class="mini"><div class="label">Bytes Out</div><div class="value" id="bytes-sent" style="font-size:18px">—</div></div>
      <div class="mini"><div class="label">Rate</div><div class="value" id="traffic-rate" style="font-size:18px">—</div></div>
    </div>
  </div>
  <div class="card" style="flex:1">
    <div class="label">Security</div>
    <div class="card-row" style="margin-top:8px">
      <div class="mini"><div class="label">Auth OK</div><div class="value green" id="auth-ok" style="font-size:16px">—</div></div>
      <div class="mini"><div class="label">Auth Fail</div><div class="value red" id="auth-fail" style="font-size:16px">—</div></div>
      <div class="mini"><div class="label">Encrypted</div><div class="value cyan" id="encrypted" style="font-size:16px">—</div></div>
    </div>
  </div>
</div>

<div class="card" id="peers-section">
  <div class="label">Peers (<span id="peer-count-label">0</span>)</div>
  <div class="peers-grid" id="peers-list"><div style="color:var(--muted);padding:8px">No peers yet</div></div>
</div>

<div class="log" id="event-log">
  <div class="line" style="color:var(--muted)">Waiting for data...</div>
</div>

<div class="footer">
  Neuron Wire Protocol · <a href="/status" style="color:var(--accent)">/status</a> ·
  <a href="/metrics" style="color:var(--accent)">/metrics</a> ·
  <a href="/health" style="color:var(--accent)">/health</a>
</div>

<script>
const NODE_URL = window.location.origin;
let logCount = 0;

async function fetchAndRender() {
  try {
    const resp = await fetch(NODE_URL + '/status');
    if (!resp.ok) throw new Error('Status: ' + resp.status);
    const d = await resp.json();

    // Status badge
    const badge = document.getElementById('status-badge');
    badge.textContent = 'ONLINE';
    badge.className = 'status-badge';

    // Update time
    document.getElementById('update-time').textContent = new Date().toLocaleTimeString();

    // Stats
    setNum('tick-rate', d.tick_rate_hz?.toFixed(0) || '0', 'cyan');
    setNum('total-ticks', d.total_ticks?.toLocaleString() || '0');
    setNum('peer-count', d.peer_count || '0', d.peer_count > 0 ? 'green' : '');
    setNum('packets-recv', d.packets_recv?.toLocaleString() || '0');
    setNum('packets-sent', d.packets_sent?.toLocaleString() || '0');
    setNum('retransmissions', d.retransmissions?.toLocaleString() || '0');
    setNum('reliable-q', d.reliable_queue_depth || '0');

    // Idle ratio
    const idleRatio = d.idle_ratio || 0;
    setNum('idle-ratio', (idleRatio * 100).toFixed(1) + '%', idleRatio > 0.5 ? 'yellow' : 'green');
    document.getElementById('idle-gauge').style.width = (idleRatio * 100).toFixed(0) + '%';
    document.getElementById('idle-gauge').className = 'fill ' + (idleRatio > 0.8 ? 'red' : idleRatio > 0.5 ? 'yellow' : 'green');

    // Traffic
    setNum('bytes-recv', formatBytes(d.bytes_recv || 0), 'cyan');
    setNum('bytes-sent', formatBytes(d.bytes_sent || 0));

    // Security (from engine health extended fields)
    setNum('auth-ok', formatNum(d.authenticated_packets || 0), 'green');
    setNum('auth-fail', formatNum(d.auth_failures || 0), 'red');
    setNum('encrypted', formatNum(d.encrypted_packets || 0), 'cyan');

    // Peers list
    const peersEl = document.getElementById('peers-list');
    document.getElementById('peer-count-label').textContent = d.peer_count || '0';
    if (d.peer_count > 0) {
      peersEl.innerHTML = Array.from({length: Math.min(d.peer_count, 50)}, (_, i) =>
        '<div class="peer"><div>Peer ' + (i+1) + '</div><div class="addr">' + (d.peer_addr || '—') + '</div><div class="lat">RTT: ' + (d.peer_rtt || '—') + 'ms</div></div>'
      ).join('');
    }

    // Event log
    const log = document.getElementById('event-log');
    const ts = new Date().toLocaleTimeString();
    const line = document.createElement('div');
    line.className = 'line';
    line.innerHTML = '<span class="ts">[' + ts + ']</span> tick=' + d.total_ticks + ' peers=' + d.peer_count + ' rx=' + d.packets_recv + ' tx=' + d.packets_sent;
    log.insertBefore(line, log.firstChild);
    logCount++;
    if (logCount > 50) {
      log.removeChild(log.lastChild);
    }

  } catch (e) {
    const badge = document.getElementById('status-badge');
    badge.textContent = 'OFFLINE';
    badge.className = 'status-badge error';
    document.getElementById('update-time').textContent = new Date().toLocaleTimeString() + ' — ' + e.message;
  }
}

function setNum(id, val, cls) {
  const el = document.getElementById(id);
  el.textContent = val;
  el.className = 'value' + (cls ? ' ' + cls : '');
}

function formatBytes(b) {
  if (b < 1024) return b + ' B';
  if (b < 1024*1024) return (b/1024).toFixed(1) + ' KB';
  return (b/1024/1024).toFixed(2) + ' MB';
}

function formatNum(n) {
  if (n >= 1e6) return (n/1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n/1e3).toFixed(1) + 'K';
  return n;
}

fetchAndRender();
setInterval(fetchAndRender, 3000);
</script>
</body>
</html>`

func Handler(w http.ResponseWriter, r *http.Request) {
	// Proxy /status, /metrics, /health to the NWP node
	if r.URL.Path == "/status" || r.URL.Path == "/metrics" || r.URL.Path == "/health" {
		if nodeURL == "" {
			http.Error(w, "NWP_NODE_URL not configured", http.StatusServiceUnavailable)
			return
		}
		proxyURL := nodeURL + r.URL.Path
		client := &http.Client{Timeout: 5 * time.Second}
		resp, err := client.Get(proxyURL)
		if err != nil {
			http.Error(w, fmt.Sprintf("upstream error: %v", err), http.StatusBadGateway)
			return
		}
		defer resp.Body.Close()
		body, _ := io.ReadAll(resp.Body)
		for k, v := range resp.Header {
			for _, vv := range v {
				w.Header().Add(k, vv)
			}
		}
		w.WriteHeader(resp.StatusCode)
		w.Write(body)
		return
	}

	// Serve the dashboard HTML
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(strings.ReplaceAll(dashboardHTML, "window.location.origin", fmt.Sprintf("%q", nodeURL))))
}
