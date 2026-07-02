# Paper 9: NWP-Vis — A Live Visualization Framework for P2P Neural Networks

**Target venue:** VIS / EuroVis / arXiv
**Status:** Planning
**Estimated pages:** 8–10

---

## Abstract

Debugging and understanding P2P neural networks is fundamentally harder than centralized ML because state is distributed across nodes, communication is asynchronous, and failure modes are emergent. We present NWP-Vis, a live visualization framework for the neuron-wire runtime that provides real-time insight into: (1) DHT topology graph with latency-weighted edges; (2) neuron activation heatmaps across the network; (3) packet flow animation with per-peer latency histograms; (4) time-travel replay of recorded experiments; and (5) live metrics dashboard with Server-Sent Events streaming. The framework is a self-contained HTML/CSS/JS dashboard (no build tools, no server-side framework) served from the node's embedded HTTP server, consuming JSON metrics over a REST API. We describe the visualization architecture, evaluate rendering performance at various network sizes, and present case studies where NWP-Vis revealed emergent behaviors invisible in log files.

## Key Claims

1. **Embedded dashboard** — every node is its own metrics server
2. **No build dependencies** — single HTML file with inline CSS/JS
3. **Time-travel replay** — recorded experiments can be replayed at any speed
4. **Revealed emergent behaviors** — case studies of bugs found only via visualization

## Outline

1. Introduction
2. Visualization Architecture
3. Dashboard Components
4. Performance
5. Case Studies
6. Related Work
7. Conclusion

## Status

- [x] Dashboard HTTP server + SSE endpoint
- [x] Dashboard HTML with DHT map, neuron graph, packet stream, charts
- [x] Metrics registry with counter/gauge/history
- [ ] Time-travel replay implementation
- [ ] Performance benchmarks at 50+/100+ nodes
