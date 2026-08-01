# Roadmap

> Three deliverables from localhost simulation to production-ready decentralized AI infrastructure.
> See [`GRANT.md`](GRANT.md) for budget, timeline, and success criteria.

---

## Deliverable 1 — Real Internet Deployment (100 Nodes, 3 Continents)

Take neuron-wire from localhost to the real Internet. Deploy across free-tier VPS in North America, Europe, and Asia.

**Why:** Every benchmark to date runs on localhost. Real networks have NAT, jitter, loss, asymmetric routing, and heterogeneous hardware. Until deployed on real infrastructure, every claim about convergence, latency tolerance, and fault recovery remains hypothetical.

### Tasks

- [ ] Deploy 100+ nodes across AWS free-tier (us-east-1, eu-west-1, ap-southeast-2)
- [ ] Measure: latency distribution (intra/inter-region), packet loss, DHT convergence time, gradient delivery rate
- [ ] Public dashboard with per-region live metrics
- [ ] Noise Protocol encryption (DTLS-style, zero-alloc)
- [ ] STUN-based NAT traversal for non-VPS nodes
- [ ] Publish negative results alongside positive ones

### Success Criteria

- 100+ nodes sustain connectivity for 7+ days
- Cross-continent gradient delivery with < 50% loss
- Public dashboard operational

---

## Deliverable 2 — Publication & Reproducible Benchmarks

**Why:** No decentralized AI infrastructure project has published a reproducible evaluation with open datasets. The field is full of claims and empty of evidence. Fixing this is a contribution regardless of outcome.

### Tasks

- [ ] Evaluation paper comparing NWP against: vanilla Kademlia, gossip SGD, federated averaging
- [ ] All 7 Python baselines on same testbed, same measurement methodology
- [ ] Open datasets (per-tick latency, throughput, convergence curves, 4 systems × 3 topologies)
- [ ] Submit to workshop (SysML, MLSys, HotOS, DSN) — registered report format preferred
- [ ] `cargo run --release -- --reproduce-all` reproduces every figure

### Success Criteria

- Paper accepted at workshop or conference
- Datasets under CC-BY license
- All figures one-command reproducible from fresh clone

---

## Deliverable 3 — Developer SDK & Ecosystem

**Why:** 8,000 lines of Rust with no onboarding path limits adoption to other systems programmers.

### Tasks

- [ ] `cargo add neuron-wire` — 5 public functions
- [ ] Python bindings via PyO3
- [ ] 5 reference applications
- [ ] Peer cache persistence + snapshot/restore
- [ ] WebRTC transport for cross-machine browser-to-browser

### Success Criteria

- `cargo add neuron-wire` → running node in 5 minutes
- Python wheel on PyPI
- 2+ external contributors from other research groups

---

## Pre-Deliverable Assets

| Asset | Status |
|-------|--------|
| DHT routing (latency-weighted Kademlia) | ✅ |
| UDP transport (3 reliability tiers, gradient decay) | ✅ |
| Engine loop (single-threaded, 6-phase, 400 KHz–1 MHz) | ✅ |
| Neural computation (forward pass + STDP) | ✅ |
| Neurogenesis + Apoptosis | ✅ |
| Identity & trust (Ed25519 signatures, Sybil resistance) | ✅ |
| Deterministic paper-mode simulator | ✅ |
| Formal mathematical model (1,760 lines) | ✅ |
| 7 Python baselines (federated, SGD, Ray, BitTensor, ...) | ✅ |
| 8 ADRs | ✅ |
| WASM browser demo | ✅ |
| CI/CD (4 workflows) | ✅ |

## Timeline (Estimated)

| Quarter | Deliverable | Milestone |
|---------|-------------|-----------|
| Q3 2026 | D1: Real Internet deployment | 20-node, three-region funded pilot (NA/EU/APAC); scaling study follows |
| Q3–Q4 2026 | D2: Publication | Paper submission, open dataset release |
| Q4 2026–Q1 2027 | D3: SDK + ecosystem | crates.io, PyPI, 5 reference apps |

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Real Internet reveals fundamental protocol flaw | Medium | Publish negative results — still a contribution |
| Low adoption despite SDK investment | Medium | Target 2 external contributors; quality over quantity |
| NAT traversal infeasible | Medium | Document constraints; provide STUN/TURN fallback |
| Paper rejected | Medium | Multiple venue pipeline; negative results also publishable |
