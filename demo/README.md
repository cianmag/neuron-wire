# NWP Demo — P2P Neural Network in the Browser

**Open one tab. Open another. They discover each other. Learn together.**
**No server. No install. Pure WebAssembly.**

This demo runs the neuron-wire P2P engine entirely inside WebAssembly. Every tab is its own node — neurons, synapses, learning, and a DHT-like discovery mechanism all running on the client side. Tabs communicate through the browser's `BroadcastChannel` API — no server, no WebSocket, no infrastructure.

## How It Works

1. **Open `www/index.html`** in two browser tabs
2. Each tab initializes a WASM node with 6 neurons connected in a sparse network
3. Tabs broadcast heartbeats every ~500ms via BroadcastChannel
4. When a heartbeat from a new tab is received → **Peer Discovered**
5. Tabs exchange neuron activations (gradients) every ~1 second
6. Each node incorporates peer activations into its forward pass
7. Hebbian STDP learning runs locally every tick
8. Close a tab → peer timeout (~3s) → **Peer Lost**

## What You See

| Visual | Meaning |
|--------|---------|
| Glowing blue circles | Neurons — brighter = higher activation |
| Blue/orange lines | Synaptic connections — color = sign, width = strength |
| Green dots (right side) | Peer tabs that were discovered |
| Blue/green flying dots | Packets being sent/received between peers |
| Orange sparks | Neuron firing events (activation > 0.8) |
| Stats panel (top-left) | Tick rate, peers alive, packets sent/recv |
| Activity log (bottom-right) | Live event stream |

## File Structure

```
demo/
├── Cargo.toml          # WASM crate config
├── src/
│   └── lib.rs          # WASM core: neuron network, DHT, learning engine
└── www/
    ├── index.html      # Minimal HTML shell
    ├── style.css       # Dark-themed styles with neon glow effects
    ├── index.js        # JS glue: BroadcastChannel, canvas renderer, animation loop
    └── pkg/            # Generated WASM output (wasm-pack build)
        ├── nwp_demo.js
        ├── nwp_demo_bg.wasm
        └── ...
```

## Building from Source

```bash
# Prerequisites: Rust with wasm32 target, wasm-pack
rustup target add wasm32-unknown-unknown
npm install -g wasm-pack   # or: cargo install wasm-pack

# Build the WASM module
cd demo
wasm-pack build --target web --out-dir www/pkg --no-opt

# Serve locally
cd www && python -m http.server 8080
# Open http://localhost:8080 in two browser tabs
```

## Technical Architecture

```
┌─────────────────────────────┐     BroadcastChannel      ┌─────────────────────────────┐
│  Tab A (WASM Node)          │  ◄─────────────────────►   │  Tab B (WASM Node)          │
│                             │                           │                             │
│  ┌───────────────────────┐  │                           │  ┌───────────────────────┐  │
│  │ Neural Network        │  │   heartbeat/gradient      │  │ Neural Network        │  │
│  │  × 6 neurons          │──┼───────────────────────────┼─►│  × 6 neurons          │  │
│  │  × 10+ synapses       │  │                           │  │  × 10+ synapses       │  │
│  │  × Hebbian STDP       │  │                           │  │  × Hebbian STDP       │  │
│  └───────────────────────┘  │                           │  └───────────────────────┘  │
│                             │                           │                             │
│  ┌───────────────────────┐  │                           │  ┌───────────────────────┐  │
│  │ Peer Discovery        │  │                           │  │ Peer Discovery        │  │
│  │  × Heartbeat ping     │──┼───────────────────────────┼─►│  × 3s timeout         │  │
│  │  × Gradient exchange  │  │                           │  │  × Activation inject  │  │
│  └───────────────────────┘  │                           │  └───────────────────────┘  │
│                             │                           │                             │
│  ┌───────────────────────┐  │                           │  ┌───────────────────────┐  │
│  │ Canvas Renderer       │  │                           │  │ Canvas Renderer       │  │
│  │ requestAnimationFrame │  │                           │  │ requestAnimationFrame │  │
│  └───────────────────────┘  │                           │  └───────────────────────┘  │
└─────────────────────────────┘                           └─────────────────────────────┘
```

## Why This Matters

This is the first working demo of the neuron-wire P2P protocol running entirely in a browser tab. It demonstrates:
- **Zero-install distribution** — works by opening a URL
- **Auto-discovery** — no central registry or bootstrap server
- **Decentralized learning** — each node runs its own Hebbian updates
- **Cross-tab communication** — via BroadcastChannel (can be extended to WebRTC for cross-machine)

## Deployment

```bash
# Serve the www/ directory on any static host
# GitHub Pages, Vercel, Netlify, or any nginx/apache
# No backend needed — it's all client-side WASM
```
