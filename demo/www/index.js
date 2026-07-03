/* ─── NWP Demo — WASM Loader · BroadcastChannel · Canvas Renderer ─ */

(function () {
  'use strict';

  // ─── Configuration ──────────────────────────────────────────────────
  const CHANNEL_NAME = 'nwp-demo-v1';
  const TAB_ID = 'nwp-' + Math.random().toString(36).slice(2, 10);
  const BG_COLOR = '#0a0a1a';
  const NEURON_BASE_RADIUS = 18;
  const NEURON_MAX_RADIUS = 36;
  const SYNAPSE_MAX_OPACITY = 0.6;
  const PEER_X_OFFSET = 120; // peer nodes sit to the right

  // ─── State ──────────────────────────────────────────────────────────
  let wasm = null;           // WASM module reference
  let channel = null;        // BroadcastChannel
  let state = null;          // Last render state from WASM
  let events = [];           // Recent visual events
  let animFrame = null;      // requestAnimationFrame ID
  let lastTime = performance.now();
  let peerCount = 0;         // Track for hiding instructions

  // ─── DOM Refs ───────────────────────────────────────────────────────
  const canvas = document.getElementById('canvas');
  const ctx = canvas.getContext('2d');
  const elNodeId = document.getElementById('stat-node-id');
  const elTick = document.getElementById('stat-tick');
  const elRate = document.getElementById('stat-rate');
  const elPeers = document.getElementById('stat-peers');
  const elNeurons = document.getElementById('stat-neurons');
  const elSent = document.getElementById('stat-sent');
  const elRecv = document.getElementById('stat-recv');
  const elEventList = document.getElementById('event-list');
  const elInstructions = document.getElementById('instructions');

  // ─── Resize ─────────────────────────────────────────────────────────
  function resize() {
    const dpr = devicePixelRatio || 1;
    const w = window.innerWidth;
    const h = window.innerHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = w + 'px';
    canvas.style.height = h + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  window.addEventListener('resize', resize);

  // ─── WASM Bridge ────────────────────────────────────────────────────
  function generateSeeds() {
    // Generate 20 random floats for WASM PRNG seeding
    return JSON.stringify(Array.from({ length: 20 }, () => Math.random()));
  }

  async function initWasm() {
    // Load WASM module
    wasm = await import('./pkg/nwp_demo.js');
    await wasm.default(); // init the WASM instance

    // Initialise the demo node
    wasm.init(TAB_ID, generateSeeds());
    elNodeId.textContent = TAB_ID.slice(4, 12);
  }

  // ─── BroadcastChannel (Cross-Tab P2P) ───────────────────────────────
  function initChannel() {
    try {
      channel = new BroadcastChannel(CHANNEL_NAME);

      channel.onmessage = (e) => {
        if (!e.data || e.data.from === TAB_ID) return;
        // Feed message into WASM engine
        if (wasm) {
          wasm.on_message(JSON.stringify(e.data));
        }
      };

      // On unload, broadcast departure
      window.addEventListener('beforeunload', () => {
        channel.postMessage({
          from: TAB_ID,
          msg_type: 'goodbye',
          tick: state?.tick || 0,
        });
      });
    } catch (err) {
      console.warn('BroadcastChannel unavailable — running standalone:', err.message);
      channel = null;
    }
  }

  // ─── Send outgoing messages ─────────────────────────────────────────
  function flushOutgoing() {
    if (!channel || !wasm) return;
    const raw = wasm.drain_outgoing();
    if (!raw || raw === '[]') return;
    try {
      const messages = JSON.parse(raw);
      for (const msg of messages) {
        channel.postMessage({
          from: TAB_ID,
          msg_type: msg.msg_type,
          tick: state?.tick || 0,
          ...JSON.parse(msg.body),
        });
      }
    } catch (e) {
      // Silently skip malformed
    }
  }

  // ─── Rendering ──────────────────────────────────────────────────────

  function drawBackground(w, h) {
    // Subtle dot grid
    ctx.fillStyle = BG_COLOR;
    ctx.fillRect(0, 0, w, h);

    ctx.fillStyle = 'rgba(255,255,255,0.015)';
    const spacing = 40;
    for (let x = 0; x < w; x += spacing) {
      for (let y = 0; y < h; y += spacing) {
        ctx.beginPath();
        ctx.arc(x, y, 1, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  function drawSynapses(synapses, neurons) {
    for (const syn of synapses) {
      const from = neurons[syn.from];
      const to = neurons[syn.to];
      if (!from || !to) continue;

      const opacity = Math.min(Math.abs(syn.weight) / 2.0, SYNAPSE_MAX_OPACITY);
      const color = syn.weight >= 0 ? `rgba(0,200,255,${opacity})` : `rgba(255,51,102,${opacity})`;

      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5 + Math.abs(syn.weight) * 0.8;
      ctx.beginPath();
      ctx.moveTo(from.x, from.y);
      ctx.lineTo(to.x, to.y);
      ctx.stroke();
    }
  }

  function drawNeuron(n, isFiring, isHovered) {
    const radius = NEURON_BASE_RADIUS + n.activation * NEURON_MAX_RADIUS;
    const alpha = 0.3 + n.activation * 0.7;

    // Glow
    ctx.shadowColor = `rgba(0,200,255,${alpha * 0.6})`;
    ctx.shadowBlur = isFiring ? 40 : 20 * alpha;

    // Main circle
    const grad = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, radius);
    grad.addColorStop(0, `rgba(200,240,255,${alpha * 0.9})`);
    grad.addColorStop(0.4, `rgba(0,200,255,${alpha * 0.6})`);
    grad.addColorStop(1, `rgba(0,100,180,${alpha * 0.3})`);

    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(n.x, n.y, radius, 0, Math.PI * 2);
    ctx.fill();

    // Inner bright core for high activation
    if (n.activation > 0.5) {
      ctx.shadowBlur = 0;
      ctx.fillStyle = `rgba(255,255,255,${(n.activation - 0.5) * 0.4})`;
      ctx.beginPath();
      ctx.arc(n.x, n.y, radius * 0.3, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.shadowBlur = 0;

    // White ring
    ctx.strokeStyle = `rgba(200,230,255,${alpha * 0.4})`;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(n.x, n.y, radius + 1, 0, Math.PI * 2);
    ctx.stroke();

    // Neuron ID
    ctx.fillStyle = `rgba(200,230,255,${alpha * 0.6})`;
    ctx.font = '10px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(`N${n.id}`, n.x, n.y);
  }

  function drawPeerConnections(peers, neurons) {
    const centerX = 200; // rough center of neuron cloud
    const centerY = 200;

    for (const peer of peers) {
      const px = peer.x + PEER_X_OFFSET;
      const py = peer.y;

      // Connection line from neuron cloud center to peer
      const opacity = peer.alive ? 0.3 : 0.05;
      ctx.strokeStyle = peer.alive
        ? `rgba(0,255,136,${opacity})`
        : `rgba(255,51,102,${opacity})`;
      ctx.lineWidth = peer.alive ? 2 : 1;
      ctx.setLineDash(peer.alive ? [] : [4, 4]);
      ctx.beginPath();
      ctx.moveTo(centerX + 100, centerY);
      ctx.lineTo(px, py);
      ctx.stroke();
      ctx.setLineDash([]);

      // Peer dot
      ctx.shadowColor = peer.alive
        ? 'rgba(0,255,136,0.3)'
        : 'rgba(255,51,102,0.1)';
      ctx.shadowBlur = peer.alive ? 20 : 5;

      const pr = peer.alive ? 12 : 8;
      let col = peer.alive
        ? `rgba(0,255,136,${0.5 + peer.avg_activation * 0.5})`
        : 'rgba(100,100,100,0.3)';
      ctx.fillStyle = col;
      ctx.beginPath();
      ctx.arc(px, py, pr, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;

      // Peer label
      ctx.fillStyle = peer.alive ? 'rgba(0,255,136,0.5)' : 'rgba(100,100,100,0.3)';
      ctx.font = '9px monospace';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';
      ctx.fillText(peer.id.slice(4, 12), px, py + pr + 4);

      // Latency indicator
      if (peer.alive && peer.latency_ticks > 0) {
        ctx.fillStyle = 'rgba(0,255,136,0.2)';
        ctx.font = '8px monospace';
        ctx.textBaseline = 'bottom';
        ctx.fillText(`${peer.latency_ticks}ms`, px, py - pr - 2);
      }
    }
  }

  function drawPackets(packets, peers, neurons) {
    const now = performance.now() / 1000;

    for (const pkt of packets) {
      const peer = peers.find(p => p.id === pkt.peer_id);
      if (!peer) continue;

      const startX = 200;
      const startY = 200;
      const endX = peer.x + PEER_X_OFFSET;
      const endY = peer.y;

      const t = pkt.progress;
      const x = startX + (endX - startX) * t;
      const y = startY + (endY - startY) * t;

      const isOut = pkt.direction === 'out';
      const color = isOut ? 'rgba(0,200,255,0.9)' : 'rgba(0,255,136,0.9)';
      const size = isOut ? 5 : 4;

      ctx.shadowColor = color;
      ctx.shadowBlur = 12;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, size, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;

      // Short trail
      ctx.fillStyle = color.replace('0.9', '0.2');
      ctx.beginPath();
      ctx.arc(x - (endX - startX) * 0.02, y - (endY - startY) * 0.02, size * 0.7, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  function drawParticles(evts, neurons) {
    // Simple particle burst for neuron firing events
    const now = performance.now();
    for (const evt of events) {
      if (evt.type !== 'NeuronFired') continue;
      const neuron = neurons[evt.neuronId];
      if (!neuron) continue;

      const age = now - evt.time;
      if (age > 500) continue;
      const t = age / 500;

      const count = 3;
      for (let i = 0; i < count; i++) {
        const angle = (i / count) * Math.PI * 2 + t * 2;
        const dist = t * 30;
        const x = neuron.x + Math.cos(angle) * dist;
        const y = neuron.y + Math.sin(angle) * dist;
        const alpha = 1 - t;

        ctx.fillStyle = `rgba(255,136,68,${alpha * 0.6})`;
        ctx.shadowColor = 'rgba(255,136,68,0.3)';
        ctx.shadowBlur = 10;
        ctx.beginPath();
        ctx.arc(x, y, 3 * alpha, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;
      }
    }
  }

  function render() {
    if (!state) return;

    const w = canvas.width / (devicePixelRatio || 1);
    const h = canvas.height / (devicePixelRatio || 1);

    drawBackground(w, h);

    const neurons = state.neurons || [];
    const synapses = state.synapses || [];
    const peers = state.peers || [];
    const packets = state.packets || [];

    // Draw synapses first (behind neurons)
    drawSynapses(synapses, neurons);

    // Draw peer connections
    drawPeerConnections(peers, neurons);

    // Draw packets in flight
    drawPackets(packets, peers, neurons);

    // Draw particles
    drawParticles(events, neurons);

    // Draw neurons
    const firingIds = new Set(
      events.filter(e => e.type === 'NeuronFired').map(e => e.neuronId)
    );
    for (const n of neurons) {
      drawNeuron(n, firingIds.has(n.id));
    }

    // Update stats
    const s = state.stats || {};
    elTick.textContent = s.tick ?? 0;
    elRate.textContent = Math.round(s.tick_rate || 0) + ' Hz';
    elPeers.textContent = (s.peers_alive ?? 0) + '/' + (s.peers_total ?? 0);
    elNeurons.textContent = s.neurons ?? 0;
    elSent.textContent = s.packets_sent ?? 0;
    elRecv.textContent = s.packets_recv ?? 0;

    // Hide instructions when a peer is found
    if ((s.peers_alive ?? 0) > 0 && peerCount === 0) {
      elInstructions.classList.add('peer-found');
    }
    peerCount = s.peers_alive ?? 0;
  }

  // ─── Animation Loop ─────────────────────────────────────────────────
  function loop(time) {
    const dt = time - lastTime;
    lastTime = time;

    if (wasm) {
      // Run one tick in WASM
      const raw = wasm.tick(dt);
      try {
        state = JSON.parse(raw);
      } catch(e) {
        // Keep previous state
      }

      // Drain events from WASM
      const evtRaw = wasm.drain_events();
      if (evtRaw && evtRaw !== '[]') {
        try {
          const newEvents = JSON.parse(evtRaw);
          for (const ev of newEvents) {
            events.push({
              type: ev.type,
              neuronId: ev.id,
              peerId: ev.id || ev.from || ev.to,
              time: performance.now(),
            });
          }
          // Keep last 50 events
          if (events.length > 50) events = events.slice(-50);

          // Update event log UI
          updateEventLog(newEvents);
        } catch(e) {}
      }

      // Send queued outgoing messages
      flushOutgoing();
    }

    render();
    animFrame = requestAnimationFrame(loop);
  }

  // ─── Event Log ──────────────────────────────────────────────────────
  function updateEventLog(newEvents) {
    for (const ev of newEvents) {
      const div = document.createElement('div');
      div.className = 'event-item';

      switch (ev.type) {
        case 'PeerDiscovered':
          div.classList.add('discover');
          div.textContent = `✦ Peer: ${ev.id?.slice(4,12) || '?'}`;
          break;
        case 'PeerLost':
          div.classList.add('lost');
          div.textContent = `✕ Lost: ${ev.id?.slice(4,12) || '?'}`;
          break;
        case 'PacketSent':
          div.classList.add('packet');
          div.textContent = `→ Packet to ${ev.to?.slice(4,12) || '?'}`;
          break;
        case 'PacketRecv':
          div.classList.add('packet');
          div.textContent = `← Packet from ${ev.from?.slice(4,12) || '?'}`;
          break;
        case 'NeuronFired':
          div.classList.add('fire');
          div.textContent = `⚡ Neuron ${ev.id || '?'} fired`;
          break;
        default:
          continue;
      }

      elEventList.prepend(div);
    }

    // Keep last 20 events in UI
    while (elEventList.children.length > 20) {
      elEventList.removeChild(elEventList.lastChild);
    }
  }

  // ─── Boot ───────────────────────────────────────────────────────────
  async function boot() {
    resize();
    initChannel();
    await initWasm();
    animFrame = requestAnimationFrame(loop);
  }

  boot();
})();
