# Research Questions

> A scientific project is defined by the questions it asks, not the code it contains.
> This document tracks every open research question for neuron-wire, with hypotheses, experimental protocols, and current evidence — or lack thereof.
> If a question cannot be answered here, it is not yet a research question — it is a speculation.

---

## RQ1 — Can decentralized (Hebbian STDP) learning converge under Internet-scale latency and packet loss?

| Field | Value |
|-------|-------|
| **Hypothesis** | Hebbian STDP can converge in a distributed setting even under realistic WAN conditions (50-300ms RTT, 0-5% loss), though convergence time will be longer than localhost. |
| **Current Status** | ❓ Unknown — all experiments to date are localhost UDP simulation |
| **Existing Evidence** | None on real networks. Localhost simulation shows convergence under zero loss, zero latency. |
| **Experiment** | Deploy 100+ nodes across North America, Europe, Asia-Pacific. Measure gradient delivery rate, convergence time of mean activation variance, DHT convergence under cross-continent latency. |
| **Independent Variables** | Node count, geographic distribution, packet loss rate (natural, not injected) |
| **Dependent Variables** | Time to convergence (variance < threshold), gradient delivery success rate, routing table freshness |
| **Success Criteria** | Network converges (activation variance across all nodes drops below 0.01) within 60 seconds of steady state, sustained for 5 minutes |
| **Failure Criteria** | Divergence (variance increases without bound), routing table collapse (nodes cannot maintain >10% of peers), or gradient loss rate > 50% |
| **Priority** | **Tier 1** — Single highest-impact question for the project |
| **Blocked By** | NAT traversal, wire encryption, deployment provisioning |

---

## RQ2 — Does latency-weighted Kademlia outperform vanilla Kademlia under realistic churn?

| Field | Value |
|-------|-------|
| **Hypothesis** | Latency-weighted bucket eviction improves lookup success rate and latency compared to vanilla Kademlia (time-based eviction) when nodes join and leave at >5%/minute churn. |
| **Current Status** | ❓ Untested — no direct A/B comparison has been run |
| **Existing Evidence** | The latency-weighted design is implemented and tested in isolation (7 unit tests), but never compared head-to-head against vanilla Kademlia on the same testbed under identical churn conditions. |
| **Experiment** | 50-node network. Two configurations: (A) NWP latency-weighted, (B) vanilla Kademlia. Run 20 trials each at churn rates of 0%, 5%, 10%, 20% nodes/minute. Measure lookup success rate and lookup latency. |
| **Independent Variables** | Routing strategy (latency-weighted vs vanilla), churn rate |
| **Dependent Variables** | Lookup success rate, average lookup latency, routing table size, stale entry ratio |
| **Success Criteria** | Latency-weighted shows ≥10% higher lookup success rate at ≥10%/min churn, with equal or lower latency |
| **Failure Criteria** | No statistically significant difference at any churn level, or latency-weighted underperforms |
| **Priority** | **Tier 1** — Direct baseline comparison; strengthens paper |
| **Blocked By** | RQ1 (need WAN deployment first), baseline harness |

---

## RQ3 — What packet loss rate causes Hebbian gossip convergence to break?

| Field | Value |
|-------|-------|
| **Hypothesis** | Hebbian STDP can tolerate up to ~20% uniform random packet loss without catastrophic convergence failure due to the exponential gradient-weight decay favoring fresh information. Beyond 30%, the signal-to-noise ratio causes divergence. |
| **Current Status** | ❓ Unknown — no systematic loss injection has been performed |
| **Existing Evidence** | The gradient decay mechanism (100ms half-life) was designed to filter stale information, which may also provide natural loss tolerance. This is a theoretical argument, not an empirical one. |
| **Experiment** | 20-node localhost network with simulated packet loss via `tc` netem. Loss rates: 0%, 1%, 5%, 10%, 20%, 30%, 50%. 20 trials per condition. Measure whether convergence occurs and how long it takes. |
| **Independent Variables** | Packet loss rate (0-50%) |
| **Dependent Variables** | Convergence success (binary), time to convergence, mean squared activation error vs. no-loss baseline |
| **Success Criteria** | Identify the loss rate at which convergence probability drops below 50% (the "break point") |
| **Failure Criteria** | System converges even under 50% loss (unexpected resilience, but valuable finding either way) |
| **Priority** | **Tier 1** — No paper should claim "fault tolerant" without this data |
| **Blocked By** | Nothing — can be done entirely on localhost with synthetic loss |

---

## RQ4 — How does neuron population size affect convergence time and resource usage?

| Field | Value |
|-------|-------|
| **Hypothesis** | Convergence time scales sub-linearly with neuron count due to sparse gossip (fanout fixed at 3 peers regardless of network size), but memory per node scales linearly with neuron count and quadratically with synapse density. |
| **Current Status** | ❓ Unknown — all experiments use default neuron counts |
| **Existing Evidence** | Architecture documents complexity claims (O(n) memory, O(k·n) compute per tick) but these are never validated empirically across more than one configuration. |
| **Experiment** | Simulator mode. Neuron populations: 10, 50, 100, 500, 1000, 5000. 10 nodes. Measure convergence time, memory usage, compute time per tick, messages per second. |
| **Independent Variables** | Neuron count |
| **Dependent Variables** | Convergence time, peak memory, avg tick duration, message throughput |
| **Success Criteria** | Empirical scaling matches claimed complexity (O(n) memory, O(k·n) compute) within 20% |
| **Failure Criteria** | Empirical scaling is super-linear (O(n²) or worse), or system crashes at 5000 neurons |
| **Priority** | **Tier 1** — Scaling claims require scaling evidence |
| **Blocked By** | Nothing — can be done entirely in simulator mode |

---

## RQ5 — Can the trust scoring system detect and isolate Byzantine (malicious) nodes?

| Field | Value |
|-------|-------|
| **Hypothesis** | Trust scoring (Ed25519 signatures + reputation-based selection) can detect simple malicious behavior (dropping all gradients, sending false gradients, flooding) with ≥90% true positive rate at ≤5% false positive rate when malicious nodes constitute ≤33% of the network. |
| **Current Status** | ❓ Untested — trust system passes unit tests but has never faced an active adversary |
| **Existing Evidence** | Trust scoring module has unit tests for scoring mechanics, but no adversarial evaluation. No Sybil attack simulation exists. |
| **Experiment** | 30-node network with 0%, 10%, 25%, 50% Byzantine nodes. Malicious behaviors: (a) drop all forwarded gradients, (b) send random gradient values, (c) flood with spurious messages, (d) Sybil (multiple identities). 10 trials per condition. |
| **Independent Variables** | Malicious proportion, attack type |
| **Dependent Variables** | True positive rate (malicious nodes identified), false positive rate (honest nodes flagged), convergence impact |
| **Success Criteria** | TP ≥ 90%, FP ≤ 5% at ≤33% malicious, with convergence still achieved |
| **Failure Criteria** | System cannot distinguish malicious from honest at better than chance, or any malicious proportion ≥10% prevents convergence |
| **Priority** | **Tier 2** — Important for security claims, but WAN deployment and paper come first |
| **Blocked By** | RQ1 (need experimental harness established) |

---

## RQ6 — Does the single-threaded 6-phase engine loop scale to 1000 nodes without modification?

| Field | Value |
|-------|-------|
| **Hypothesis** | The single-threaded, non-blocking engine loop can sustain 1000 nodes at ≥100 Hz tick rate before the UDP socket becomes the bottleneck (a single `recv_from()` cannot drain 1000 nodes' messages within one tick at standard message rates). |
| **Current Status** | ❓ Unknown — max tested configuration is ~20 nodes |
| **Existing Evidence** | Engine loop design documents claim ~400 KHz–1 MHz tick rate on commodity hardware at low node counts. The UDP drain phase is documented as the eventual bottleneck. No empirical bottleneck identification exists. |
| **Experiment** | Simulator mode with simulated UDP transport. Scale: 10, 50, 100, 500, 1000 nodes. Measure tick rate, drain time per tick, CPU usage, message drop rate at socket. |
| **Independent Variables** | Node count, message rate per node |
| **Dependent Variables** | Tick rate, drain time, CPU utilization, dropped messages |
| **Success Criteria** | System maintains ≥100 Hz tick rate at 1000 nodes with <1% message drop |
| **Failure Criteria** | Tick rate drops below 10 Hz, or >10% of messages dropped due to socket saturation |
| **Priority** | **Tier 2** — Scaling limits inform parallel engine design, but not needed for initial publication |
| **Blocked By** | RQ4 (neuron scaling experiment), RQ1 |

---

## RQ7 — How does gradient-weight decay compare to uniform (non-decayed) gradient exchange?

| Field | Value |
|-------|-------|
| **Hypothesis** | Exponential gradient-weight decay (100ms half-life) improves convergence speed compared to uniform weighting when the network has heterogeneous latency (variance > 50ms), because stale gradients receive proportionally less influence. Under zero-latency conditions, decay has no effect (positive or negative). |
| **Current Status** | ❓ Untested — decay is the only implemented behavior; no toggle exists to disable it |
| **Existing Evidence** | The decay formula is mathematically motivated (exponential weighting is optimal for variance reduction in distributed consensus), but this is a theoretical argument from signal processing, not an empirical result for neural gradient exchange. |
| **Experiment** | 20-node network with artificial latency variance (10-200ms per link). Two configurations: (A) decay enabled, (B) decay disabled (uniform weighting). 30 trials each. Measure convergence time and steady-state variance. |
| **Independent Variables** | Decay enabled/disabled, latency variance magnitude |
| **Dependent Variables** | Time to convergence, steady-state activation variance, bandwidth |
| **Success Criteria** | Decay improves convergence time by ≥15% under high latency variance (≥100ms spread). No significant difference under low variance. |
| **Failure Criteria** | Decay has no effect, or harms convergence, under any tested condition |
| **Priority** | **Tier 2** — Important scientific question about the architecture, but less impactful than WAN deployment |
| **Blocked By** | Feature: toggle to disable decay (estimated <50 lines) |

---

## RQ8 — Is the learning signal (Hebbian STDP gradients) distinguishable from noise after crossing 3+ hops on the gossip mesh?

| Field | Value |
|-------|-------|
| **Hypothesis** | By the 4th gossip hop, the gradient signal-to-noise ratio drops below 1.0, meaning gradients from nodes farther than 3 hops are indistinguishable from random noise. This would imply the effective network diameter for learning is ≤3 hops regardless of physical network size. |
| **Current Status** | ❓ Unknown — gossip fanout diameter has never been measured |
| **Existing Evidence** | Sparse gossip sends to up to 3 peers per tick. No analysis exists of how many hops a gradient survives before its weight decays below significance. |
| **Experiment** | 50-node network. Tag gradients with source node ID. Track per-gradient weight as it propagates. Measure SNR (signal vs. average gradient from random permutation) at each hop distance. 10 trials. |
| **Independent Variables** | Hop distance (1-10) |
| **Dependent Variables** | Gradient weight, signal-to-noise ratio, probability of influencing destination node's weights |
| **Success Criteria** | Identify the hop distance at which SNR crosses 1.0 |
| **Failure Criteria** | SNR stays above 1.0 at all distances (surprising, would imply unbounded propagation) — still a publishable negative result |
| **Priority** | **Tier 2** — Deeply interesting, feeds into paper discussion section |
| **Blocked By** | RQ1 (need experimental harness) |

---

## Experimental Protocol Template

For every new research question added to this document, the following fields are required:

```yaml
question: A falsifiable question ending with "?"
hypothesis: A specific, testable prediction
current_status: ❓Unknown | 📊In Progress | ✅Complete | ❌Falsified
existing_evidence: What we already know (or don't)
experiment: Concise description of the procedure
independent_variables: What we change
dependent_variables: What we measure
success_criteria: What confirms the hypothesis
failure_criteria: What falsifies the hypothesis
priority: Tier 1-10 (from PRIORITIZATION.md)
blocked_by: Prerequisite experiments or features
```

If a proposed question cannot be expressed with all fields filled, it is not yet specific enough to be a research question.

---

## Priority Index

| Priority | Question | Status |
|----------|----------|--------|
| Tier 0 | **RQ9 — Hebbian STDP learning benchmark** | **❓ Unknown** |
| Tier 1 | RQ1 — WAN convergence | ❓ Unknown |
| Tier 1 | RQ2 — Latency-weighted vs vanilla Kademlia | ❓ Untested |
| Tier 1 | RQ3 — Packet loss tolerance | ❓ Unknown |
| Tier 1 | RQ4 — Neuron scaling | ❓ Unknown |
| Tier 2 | RQ5 — Byzantine detection | ❓ Untested |
| Tier 2 | RQ6 — Engine loop scaling | ❓ Unknown |
| Tier 2 | RQ7 — Gradient decay effect | ❓ Untested |
| Tier 2 | RQ8 — Gradient SNR over gossip hops | ❓ Unknown |

|---

## RQ9 — Does Hebbian STDP over P2P gradient gossip converge to the correct weights in a distributed regression task? (THE LEARNING BENCHMARK)

| Field | Value |
|-------|-------|
| **Hypothesis** | N nodes (3–100) running Hebbian STDP with gradient gossip can learn a linear mapping y = w · x + b from streaming data, converging to within 1% of known ground-truth weights within 500 epochs, albeit requiring more communication epochs than centralized SGD to reach equivalent MSE. |
| **Current Status** | ❓ Unknown — no end-to-end learning benchmark has ever been executed on the system |
| **Existing Evidence** | Forward pass propagates activations correctly (7 unit tests). STDP weight update rule is mathematically correct (8 unit tests). Gradient gossip serialization/deserialization round-trips correctly (3 tests). But the three subsystems have never been evaluated as a pipeline. |
| **Experiment** | Distributed linear regression protocol defined in PROJECT_INTELLIGENCE.md §8 — 10 000 samples, 80/10/10 split per node, 32-sample batches, Hebbian STDP (lr=0.01, λ=0.999), gossip every 500 ticks to 3 peers, compare against centralized SGD and federated averaging (scikit-learn). |
| **Independent Variables** | Number of nodes (3, 10, 30, 100), gossip interval (100, 500, 2000 ticks), learning rate (0.001, 0.01, 0.1) |
| **Dependent Variables** | Epochs to convergence (MSE < 0.01 × ground truth variance), final test MSE, communication cost per node (bytes), wall-clock time per epoch, packet count |
| **Success Criteria** | All node configurations achieve MSE < 0.01 × ground truth variance within 500 epochs. Convergence time scales sub-linearly with node count. Decentralized Hebbian-STDP requires ≤ 3× the communication of centralized SGD. |
| **Failure Criteria** | System never converges (MSE stays flat or diverges), or convergence requires > 500 epochs at any configuration. Hebbian-STDP requires > 10× the communication of centralized SGD. Nodes converge to different weights. |
| **Priority** | **Tier 0** — Single highest-impact scientific gap. Every claim about "collaborative learning" depends on this question being answered. This benchmark must be executed before any paper submission. |
| **Blocked By** | Harness code to wire: (a) synthetic data generation into the neural graph, (b) per-node MSE logging at configurable intervals, (c) epoch tracking independent of tick rate, (d) centralized SGD and federated averaging baselines via Python. Estimated 200–400 lines of Rust + Python. |
