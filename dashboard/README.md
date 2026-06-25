# Public Benchmark Dashboard

**URL:** https://neuron-wire-dashboard.vercel.app  
**Repo:** https://github.com/cianmag/neuron-wire  
**Deployed:** 2026-06-25

## What it shows
- **Network topology animation** — 12-node orbiting force-directed graph with pulse effect
- **4 live Chart.js charts** — convergence time, bandwidth scaling, peer discovery (5-node + 50-node)
- **Stats row** — animated counters: 50 max nodes, 100% conv rate, 3.25s avg conv, 36.5 Mbps peak
- **Full results table** — 21 trials across 5/10/25/50 node scales
- **Churn & routing stats** — apoptosis deaths = 0, packet delivery = 100%, K=20 buckets
- **Live uptime clock** — shows how long the page has been open

## Tech
- Pure HTML+JS+CSS, Chart.js from CDN, zero build step
- Deployed to Vercel in 7s with zero config (vercel.json)
- GitHub Pages also available via https://cianmag.github.io/neuron-wire/dashboard/
