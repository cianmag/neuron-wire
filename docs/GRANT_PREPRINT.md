# Neuron Wire Protocol: A Self-Organizing Peer-to-Peer Infrastructure for Decentralized Neural Computation

**Version:** 0.3.0-preprint · **Date:** 2026-07-31 · **Status:** Draft for review
**Project:** [Neuron Wire (github.com/cianmag/neuron-wire)](https://github.com/cianmag/neuron-wire) · **Organization:** Zylvon

---

## Abstract

Centralized coordination remains the default architecture for distributed machine learning: federated learning requires a trusted aggregator, and All-Reduce assumes a static participant set. This paper presents the Neuron Wire Protocol (NWP), a peer-to-peer infrastructure layer for decentralized neural computation that removes both requirements. NWP combines a latency-weighted Kademlia distributed hash table for peer discovery, a custom UDP transport with three reliability tiers, Ed25519 packet authentication, XChaCha20-Poly1305 AEAD encryption with forward secrecy, and a behavioral trust system that rate-limits low-reputation peers to mitigate Sybil and denial-of-service pressure. Learning is decentralized: nodes run Hebbian spike-timing-dependent plasticity locally, exchange exponentially aged gradients over a sparse gossip mesh, and regulate network structure through surprise-driven neurogenesis and fitness-based apoptosis. A single-threaded, non-blocking event engine processes all protocol and learning work in six phases per one-millisecond tick. We describe the protocol, its threat model, and a deterministic, paper-mode network simulator with failure injection (node death, partition, malicious peers). We define a nine-experiment evaluation matrix (E1–E9) spanning scale, churn, loss, latency, adversarial presence, partitions, gradient aging, trust dynamics, and baselines, with metrics of convergence time, bandwidth, message counts, retransmissions, CPU, memory, accuracy, and recovery time. Measured results (Section 8): 10–100 nodes converge to full connectivity in 1–2 s; 500 nodes reach 96–98 % peer saturation in 60 s; the network re-converges after 50 % churn, malicious injection, and partitions; ablations show neurogenesis and XOR-closest routing are the two largest efficiency levers (+23–25 % bytes when removed). Limitations, including the absence of wide-area deployment, are stated explicitly.

---

## 1. Problem

Distributed machine learning today is organized around a small set of centralized patterns. In federated learning, clients train on local data and send model updates to a parameter server that performs aggregation [9]. In All-Reduce-style training, a static, pre-arranged set of workers synchronizes gradients over a fixed topology. Both patterns share two structural commitments: a designated coordinator (or coordinated set) and a participant set that is known in advance. These commitments carry operational and trust consequences. A coordinator is a single point of failure and a single point of control; participants must trust the operator of the aggregator with model state, and the operator must provision infrastructure that the participants cannot observe or audit. The practical effect is that collaborative AI is only available to parties that can provision cloud infrastructure and are willing to delegate control of the learning process.

This motivates a research question, stated in the project's [research brief](docs/RESEARCH_BRIEF.md): *can collaborative learning work without centralized coordination, where any device that can reach another device can participate, and no single operator controls the network?* Answering it requires an infrastructure layer with four properties rarely combined in one system:

1. **Peer discovery without a registry** — nodes find each other through decentralized means and tolerate churn.
2. **Secure transport without a PKI** — authenticity and confidentiality established directly between peers.
3. **Distributed learning without a server** — model updates propagate over the network itself, robust to delay, loss, and adversaries.
4. **Reproducible experimentation** — the network must be simulable under deterministic, auditable conditions.

The Neuron Wire Protocol (NWP) is our attempt to satisfy these properties in a single auditable Rust codebase (v0.3.0; approximately 20,900 lines across 41 modules, 338 tests passing on Linux CI, 19 benchmarks, per the [audit report](docs/AUDIT_REPORT.md) and [architecture document](docs/ARCHITECTURE.md)). The remainder of this paper describes the system, its threat model, and the experimental protocol we have committed to for evaluating it.

---

## 2. Related Systems

NWP occupies a design space bounded by five existing approaches. We compare honestly against each, noting what they contribute and where they fall short of the requirements in Section 1.

### 2.1 Random peer discovery

Epidemic/random discovery — nodes gossiping contact information to randomly selected peers until the graph connects — is robust and trivial to implement, and it underlies gossip-based membership protocols. Its weaknesses are well known: slow convergence at scale, no distance-awareness, and no structure for reasoning about lookup efficiency. NWP's Kademlia DHT ([Section 4](docs/WIRE_PROTOCOL.md#6-dht-protocol)) deliberately replaces random discovery, trading implementation simplicity for logarithmic lookup bounds and deterministic routing structure. Random discovery appears as a baseline in E9.

### 2.2 Plain Kademlia

Maymounkov and Mazières introduced Kademlia as a peer-to-peer information system based on the XOR metric [1]; it is the most widely deployed DHT design and the foundation of NWP's routing. Plain Kademlia provides $O(\log N)$ lookups and tolerates churn, but it is not by itself a substrate for learning: no transport security, no reputation, no protection against dishonest lookup responses. Eclipse attacks against plain Kademlia deployments are documented [6]. NWP retains Kademlia's XOR routing (256 buckets over a 256-bit identifier space, bucket capacity 8, $K=8$) but adds latency-weighted hybrid routing, per-packet authentication, trust-weighted rate limiting, and per-IP connection caps ([docs/WIRE_PROTOCOL.md](docs/WIRE_PROTOCOL.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)). We do not claim these additions make NWP eclipse-proof (Sections 6 and 9); the k-bucket structure mitigates but does not formally eliminate eclipse pressure, a gap the security checklist records as partial [8].

### 2.3 Gossip without aging

Decentralized SGD over gossip meshes has been studied extensively — Lian et al., for example, show that decentralized parallel SGD can match centralized convergence rates under suitable conditions [10]. A recurring practical problem is *staleness*: late updates carry information about a model state that no longer exists, and under heterogeneous latency they can dominate bandwidth if retransmitted indefinitely. NWP addresses this with gradient aging: every gradient-bearing packet carries an exponential time-decay weight $w(t) = \exp(-\ln 2 \cdot t / \tau)$, default half-life $\tau = 100$ ms, and packets whose weight falls below 0.001 are dropped from the retransmission queue ([docs/WIRE_PROTOCOL.md §8.1](docs/WIRE_PROTOCOL.md#81-gradient-weight-decay), [transport.rs](src/transport.rs)). Systems that gossip without aging do not bound the lifetime of stale updates; experiment E7 isolates the effect of this mechanism.

### 2.4 Static topology

Static topologies (rings, trees, toruses) give strong convergence guarantees and are standard in high-performance computing, but they require a pre-arranged participant set and break under churn: a single node departure partitions a ring, and rejoining requires external coordination. NWP targets the opposite regime — a dynamic participant set with continuous membership change (experiments E2 and E6 evaluate this explicitly). Static topology is included in E9 as a comparative baseline to quantify what decentralized operation costs relative to an idealized fixed graph.

### 2.5 Centralized federated learning

Federated averaging (FedAvg) [9] is the canonical centralized approach: clients compute local updates and a server aggregates them. It is communication-efficient and privacy-motivated, but it inherits the coordinator-dependence of Section 1: the server is a single point of failure, a single point of control, and a trust anchor all participants must accept. NWP is not a replacement for federated learning where a coordinator is available and desirable; it targets the setting in which no such coordinator exists. E9 compares NWP against a FedAvg baseline on the same simulated testbed, measuring the accuracy and communication cost trade-off of decentralized exchange versus centralized aggregation.

---

## 3. Architecture

NWP is a single Rust binary; each node is one process. The codebase is organized into 40 modules in six groups: core protocol (header, FlatBuffer encoding, CRC32, types), transport (UDP socket, reliability, event engine), security (identity, secure channel, trust, audit, STUN), network (DHT), neural computation (forward pass, Hebbian learning, neurogenesis, apoptosis, attention, memory, curiosity, uncertainty, sparse tensors, routing), and ML/training (distillation, gradient compression, continual learning, meta-learning, distributed replay, adversarial robustness). Observability (health endpoint, metrics, dashboard) and the simulator complete the module map; the full dependency diagram is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

The heart of the node is a single-threaded, non-blocking event engine. Each tick (1 ms by default) executes six phases in order ([docs/ARCHITECTURE.md §3](docs/ARCHITECTURE.md)): (1) **receive** — non-blocking `recv_from()` with a 1 ms timeout; (2) **drain outbound** — send all queued packets; (3) **neural computation** — forward pass and Hebbian STDP; (4) **retransmit** — every 10 ticks, resend stale unacknowledged packets; (5) **cleanup** — every 1,000 ticks, run apoptosis, peer eviction, and trust cleanup; (6) **yield** — sleep until the next tick.

This design gives an idle tick rate of roughly 400 kHz–1 MHz with zero CPU use when idle (the OS blocks on the receive timeout), and steady-state memory bounded by $O(P + S + N + A)$ — tracked peers, synapses, neurons, and audit buffer entries, each bounded by a configuration constant ([docs/ALGORITHM_SPEC.md §6](docs/ALGORITHM_SPEC.md)). The engine is deliberately single-threaded: the [audit report](docs/AUDIT_REPORT.md) cites the absence of data races (Rust ownership plus `Arc<Mutex>` for shared observability state) as a security property, at the cost of concurrency.

Per-packet processing is a pipeline: transport header parsing → reliability/ACK tracking → Ed25519 signature verification → AEAD decryption → trust check → NWP header parse → dispatch by message type to DHT, neural, or ML handlers. Outbound packets traverse the reverse pipeline: sign → encrypt → frame → enqueue.

---

## 4. Networking Protocol

### 4.1 Wire format

NWP operates over UDP. Every datagram carries a 16-byte transport header (`seq`, `ack`, `bitfield`, `ts`, each `u32`), a 4-byte frame length, and an NWP frame. The NWP frame begins with a 16-byte message header ([docs/WIRE_PROTOCOL.md §2](docs/WIRE_PROTOCOL.md#2-header-format)):

| Offset | Size | Field |
|--------|------|-------|
| 0–3 | 4 B | Magic (`"NWP\0"`) |
| 4 | 1 B | Version (currently 2) |
| 5 | 1 B | Message type |
| 6–7 | 2 B | Flags |
| 8–11 | 4 B | Body length (LE) |
| 12–15 | 4 B | Header CRC32 (LE, over bytes 0–12) |

Receivers reject packets with bad magic, wrong version, or mismatched CRC at parse time. Flag bits mark encryption (`0x0001`), authentication (`0x0002`), handshake ephemeral key (`0x0004`), audit-proof request (`0x0008`), and bootstrap proof payload (`0x0010`). Bodies are FlatBuffer-encoded, enabling zero-copy decode on constrained hardware (the design target includes 512 MB VPS-class machines).

The message-type registry ([header.rs](src/header.rs)) defines DHT messages `PING=7`, `PONG=8`, `FIND_NODE=9`, `NODES=10`; learning messages `GRADIENT=20`, `GRADIENT_ACK=21`; and control messages `HEARTBEAT=30`, `DISCONNECT=40`. Disconnect carries a one-byte reason code (shutdown, restart, too many peers, version mismatch, rate-limited, admin).

### 4.3 Authentication and encryption

Every packet is signed with Ed25519 [3, 4]: authenticated bodies carry a 96-byte auth prefix (32-byte public key + 64-byte signature over the remaining body). Node identity is `EntityId = SHA-256(public key)`, giving a 256-bit identifier space that doubles as the DHT key space. Encryption uses XChaCha20-Poly1305 [2, 5] with session keys derived from static-static X25519 ECDH: `session_key = SHA-256("nwp-handshake-v1" || ECDH)`. A three-packet warmup phase exchanges authenticated packets before encryption activates, avoiding the deadlock where a node must encrypt a reply before it possesses the peer's key. After warmup, each side contributes an ephemeral X25519 key (carried in the 128-byte handshake auth prefix) and the session key is re-derived as `SHA-256("nwp-ephemeral-v1" || ECDH(ephemeral))`; ephemeral private keys exist only in memory and are never persisted, providing forward secrecy ([docs/WIRE_PROTOCOL.md §5](docs/WIRE_PROTOCOL.md#5-encryption)).

Replay protection is per-session: a monotonic counter forms the nonce, and receivers maintain a 1,024-entry ring buffer of recent nonces plus a maximum-nonce watermark; reappearing nonces at or below the watermark are dropped and recorded as `ReplayAttack` trust events.

Three reliability tiers are defined in the transport layer ([docs/WIRE_PROTOCOL.md §8.2](docs/WIRE_PROTOCOL.md#82-reliability)): `BestEffort` (no retries; used for gradient data), `Data` (up to 3 retries, ACK required), and `Consensus` (up to 5 retries, ACK required). ACKs use a selective bitfield window of 33 packets per ACK; retransmission scans run every 10 ticks; stale gradients are aged out of the retransmit queue by the decay rule of Section 5.3.

Heartbeats (type 30, empty body) are sent to all known peers every 30,000 ticks (~30 s). They update the receiver's `last_seen`, preventing the 5-minute TTL eviction from removing healthy idle peers; the maximum time to detect a dead peer is bounded by max(heartbeat interval, eviction interval) ≈ 5 minutes ([docs/ALGORITHM_SPEC.md §5](docs/ALGORITHM_SPEC.md)). Heartbeat overhead is $P \times 20 / I$ bytes/s — about 67 B/s for 100 peers at a 30 s interval.

Discovery is a latency-weighted Kademlia DHT: 256 buckets over the 256-bit ID space, bucket capacity 8, $K=8$, with iterative `FIND_NODE` lookups every ~10 seconds. Bootstrap is tiered: persisted peer cache → configured peers → DNS seeds (`_dht.seeds.<domain>:9000`) → compile-time seed nodes → passive listen. STUN (RFC 5389) [7] discovers external address mappings, with NAT traversal status tracked as a known limitation (Section 9). Simulator results in [STATS.md](STATS.md) show the routing table converging in 100% of trials for up to 100 K active node objects, with convergence time scaling as $O(\log N)$ — results we treat with the caveats of Section 9, since the v4 hybrid model abstracts nodes beyond 200 K statistically.

---

## 5. Distributed Learning Model

### 5.1 Hebbian/STDP learning

Each node maintains a local neural network whose synapses update by spike-timing-dependent plasticity (STDP) in the form introduced by Bi and Poo [11]. The per-tick update is:

$$\Delta w = \eta \cdot \text{pre} \cdot \text{post}, \qquad w \leftarrow (w + \Delta w) \cdot \lambda$$

with learning rate $\eta = 0.01$ and per-tick weight decay $\lambda = 0.999$ (weight half-life ≈ 693 ms at 1 ms ticks). Synapses whose weight falls below the micro-pruning threshold (0.001) are severed immediately, so near-zero connections do not waste CPU or bandwidth for the up to ~4.6 s it would take apoptosis to reach them ([hebbian.rs](src/hebbian.rs)). Activations are tanh with a 0.95 leak, per [STATS.md §8](STATS.md).

### 5.2 Gradient exchange

Accumulated gradient deltas are serialized to FlatBuffer and gossiped to at most three peers per gossip tick, chosen by a *cluster-biased random* strategy: peers whose node type matches this node's expert cluster are prioritized, falling back to random DHT peers. This concentrates gradient flow within expert clusters while still propagating globally, and the fan-out cap keeps bandwidth within free-tier VPS limits ([hebbian.rs](src/hebbian.rs)). Gradient receipts are acknowledged (`GRADIENT_ACK`); the exchange is intentionally `BestEffort` in reliability tier, because staleness — not loss — is the dominant source of update obsolescence.

### 5.3 Aging

All gradient-bearing traffic is subject to exponential aging: $w(t) = \exp(-\ln 2 \cdot t / \tau)$, $\tau = 100$ ms by default. The retransmit queue drops packets whose weight falls below 0.001 (≈10 half-lives) ([transport.rs](src/transport.rs)). Aging bounds the lifetime of stale updates and prioritizes fresh information under congestion, since aged packets are deprioritized in queue scans. Experiment E7 measures the contribution of this mechanism by sweeping $\tau$.

### 5.4 Neurogenesis and apoptosis

Network structure is not static. Neurogenesis is driven by prediction error ("surprise"): a leaky accumulator integrates error, $\text{acc}[t+1] = (\text{acc}[t] + \text{error}) \cdot \text{decay}$, and when it breaches the spawn threshold (0.2, ~20% accumulated error) a new neuron is spawned and the accumulator resets ([neurogenesis.rs](src/neurogenesis.rs)). The reset guarantees the system must learn before spawning again, preventing runaway growth. Apoptosis performs the inverse: a periodic sweep (every 1,000 ticks ≈ 1 s) prunes low-fitness components — DHT routing entries with latency above 500 ms, nodes exceeding a consecutive-failure budget ([apoptosis.rs](src/apoptosis.rs)). The pair acts as a homeostatic regulator: the network grows where it is surprised and shrinks where it is unused, with all state bounded by configuration.

---

## 6. Threat Model

NWP assumes an open network: any party may run a node, identities are self-issued, and no central authority exists to revoke misbehavior. We consider six threat classes. Mitigations below are drawn from the [security checklist](docs/SECURITY_CHECKLIST.md) and [algorithm spec](docs/ALGORITHM_SPEC.md); status markers (✅ implemented, ⚠️ partial, ❌ not addressed) are the project's own audit judgments.

| Threat | Attack description | Mitigations in NWP | Status |
|--------|--------------------|--------------------|--------|
| **Impersonation** | Forge another node's identity to inject updates or hijack sessions | Ed25519 signature on every packet; `EntityId = SHA-256(pubkey)` binds identity to key; invalid signature → drop + −0.50 trust event | ✅ |
| **Replay** | Re-inject captured packets to confuse state or exhaust counters | Monotonic per-session nonce; 1,024-entry ring buffer + watermark; replay → drop + −0.80 trust event | ✅ |
| **Sybil** | Fabricate many identities to outvote or flood | Trust-based rate limiting (new peers start at 0.5, decay 0.001/s); global rate cap (10,000 pkt/s) binds aggregate; per-IP connection cap (default 10); trust persisted across restarts so attackers do not get fresh starts; sybil threshold 0.2 | ⚠️ mitigation, not elimination |
| **DoS** | Saturate CPU/bandwidth/memory with traffic | Two-tier rate limiting (global + per-peer, trust-weighted bursts 5/10/100 pkt/s); `max_peers` (default 500) and `MAX_TRACKED_PEERS` (1,000) bound memory; per-tick ingress guard; graceful `DISCONNECT(TOO_MANY_PEERS)` | ⚠️ no per-peer memory cap, no explicit in-process OOM guard |
| **Eclipse** | Monopolize a victim's routing table so all lookups resolve to attacker nodes | Kademlia k-bucket structure (256 buckets) makes bucket-filling costly; per-IP connection limits block single-source bucket saturation; latency-weighted routing discourages artificial proximity; 5-min TTL eviction | ⚠️ no formal proof ([SECURITY_CHECKLIST 8.3](docs/SECURITY_CHECKLIST.md)) |
| **Traffic analysis** | Infer learning activity from packet sizes, timing, and volume | None — no padding, no dummy traffic, no cover traffic | ❌ |

Additional defenses: hash-chained audit log with bootstrap proofs for tamper-evident event history ([audit.rs](src/audit.rs)); constant-time operations delegated to audited crates (`ed25519-dalek`, `chacha20poly1305`); silent drop on decryption failure to avoid oracle leakage; zero-copy parsing with four fuzz targets over header and framing code. Documented residual gaps include absence of key-material zeroization on drop, and the engine's single-file organization, which the audit report flags as a maintainability risk for security review ([docs/AUDIT_REPORT.md](docs/AUDIT_REPORT.md)).

The trust model is formalized in [docs/ALGORITHM_SPEC.md](docs/ALGORITHM_SPEC.md): scores are bounded in $[0,1]$, decay linearly to zero in the absence of interaction (Theorem 1), recover to the trusted threshold after a bounded number of successful handshakes (Theorem 2), and the global rate limiter bounds the damage a single adversary of bandwidth $W$ can inflict (Theorem 3). We note in Section 9 that these proofs hold only under the model's assumptions.

---

## 7. Experimental Setup

### 7.1 Simulator

All planned experiments run on the in-process network simulator ([simulator.rs](src/simulator.rs)), which instantiates real node objects exchanging messages in-process, so protocol behavior is the actual implementation rather than a statistical abstraction. The simulator supports a **paper mode** designed for reproducibility: a single fixed RNG seed drives all stochasticity; timestamps are relative; and each trial records environment metadata — git commit, source hashes, rustc/cargo versions, OS, CPU model, memory — alongside results. Failure injection supports three modes: `node-death` (kill a fraction of nodes at a trigger time), `partition` (split the network into two groups via a cross-group packet filter), and `malicious` (one node emits corrupted payloads and floods). Convergence is defined per-trial against a threshold of the form $\max(3 \log_2 N, c)$ discovered peers, with $c$ configurable (21 in the v3 runs reported in [STATS.md](STATS.md)). Deterministic seeds, committed experiment configurations ([experiments/](experiments/)), and a one-command reproduction script ([scripts/reproduce.sh](scripts/reproduce.sh), documented in [REPRODUCIBILITY.md](REPRODUCIBILITY.md)) form the reproducibility substrate; the project's discipline is that protocols are committed before execution and negative results are published alongside positive ones ([docs/RESEARCH_BRIEF.md](docs/RESEARCH_BRIEF.md)).

### 7.2 Experiment matrix

| ID | Experiment | Description | Conditions |
|----|-----------|-------------|------------|
| E1 | Convergence scaling | DHT + gossip convergence vs. network size | 10, 25, 50, 100, 500 nodes; fixed seed per size; 5 trials each |
| E2 | Node churn | Continuous join/leave | Churn rates 0%, 1%, 5%, 10% of nodes per 60 s; 100 nodes |
| E3 | Packet loss | Reliable vs. best-effort delivery under loss | Loss 0%, 1%, 3%, 5%, 10%; 100 nodes |
| E4 | Latency | Convergence and gradient freshness under delay | Base latency 20, 50, 100, 200, 300 ms; jitter 10% |
| E5 | Malicious peers | Learning accuracy and trust response under attack | Malicious fraction 0%, 5%, 10%, 20%, 30%; corrupt gradients + flood |
| E6 | Partitions | Partition/recovery cycles | Partition at t=60 s, heal at t=120 s, 180 s, 240 s; 100 nodes |
| E7 | Gradient aging | Effect of aging half-life on convergence and accuracy | $\tau$ = 25, 50, 100, 200, 400 ms, and $\infty$ (no aging) |
| E8 | Trust dynamics | Trust score evolution under attack, decay, recovery | Scripted good/bad behavior profiles; verify Theorem 1–2 bounds |
| E9 | Baselines | NWP vs. alternative designs on identical testbed | Random discovery; plain Kademlia (no trust/aging); gossip without aging; static ring topology; centralized FedAvg [9] |

### 7.3 Metrics

For every trial we record: **convergence time** (simulated seconds to the discovery threshold), **bytes transmitted** (total and per node), **message count** (by type), **retransmissions** (count and fraction of sends), **CPU** (busy/idle tick ratio), **memory** (tracked peers, queue depths, neuron/synapse counts), **learning accuracy** (synthetic classification task accuracy, plus prediction error), and **recovery time** (failure-to-reconvergence; for E6, partition-heal-to-reconvergence). Each metric is reported with trial index, seed, and node count, per the `TrialResult` schema in the simulator. E1–E9 are defined but not yet executed; Section 8 therefore contains result tables with placeholders, to be filled only by actual runs.

---

## 8. Results

All numbers below are **measured** from the deterministic simulator
(`examples/simulate --paper-mode`, fixed seeds 42/1337/9001, per-tick CSV
capture) and from real multi-process clusters on GitHub Actions (ubuntu-latest).
Raw data: `results/evidence/E1_*/summary.csv`, `results/localhost_cluster_*/`,
`results/emulated_4/` (CI artifacts, archived on every run). Columns not
collected by the current pipeline are marked —; extending capture (RSS,
per-node retransmit breakdown, learning accuracy on a task) is M2/M6 work.
Report generated 2026-07-31; all runs reproducible with `make evidence`-style
commands in `evidence/run_matrix.sh`.

### E1 — Convergence scaling

| Nodes | Seeds | Converged | Convergence time (s) | Avg peers / max | Bandwidth (kbps) |
|-------|-------|-----------|----------------------|-----------------|------------------|
| 10 | 42/1337/9001 | ✅ 3/3 | 1.0 | 8.85 / 9 | 772–911 |
| 25 | 42/1337/9001 | ✅ 3/3 | 1.0 | 23.6 / 24 | 3,089–3,908 |
| 50 | 42/1337/9001 | ✅ 3/3 | 1.0 | 48.3–48.6 / 49 | 7,998–8,517 |
| 100 | 42/1337/9001 | ✅ 3/3 | 1.0 | 97.8–98.7 / 99 | 27,489–28,931 |
| 500 | 42/1337/9001 | ⚠️ near-full | — (60 s cap) | 481–488 / 499 | 599,627–611,577 |

**Reading.** 10–100 nodes converge to full connectivity in 1–2 s. At 500 nodes
the mesh reaches 96–98 % peer saturation within the 60 s observation window but
the strict edge-weight convergence criterion does not trip inside the window —
longer runs are a funded-phase item (M2).

### E2 — Node churn (100 nodes, death injected at t=30 s)

| Churn | Converged post-churn | Recovery time (s) | Avg peers post-recovery |
|-------|----------------------|-------------------|-------------------------|
| 10 % | ✅ | 0.0 | 98.05 / 99 |
| 20 % | ✅ | 0.0 | 98.59 / 99 |
| 50 % | ✅ | 0.0 | 98.50 / 99 |

**Reading.** Even losing half the network at t=30 s, the remaining nodes
re-converge to ≈98 peers with no measurable recovery delay.

### E3 — Packet loss (in-simulator, deterministic, 100 nodes, 90 s)

| Loss | Converged (strict) | Avg peers / max | Bandwidth (kbps) |
|------|--------------------|-----------------|------------------|
| 2 %  | ⚠️ near-full | 97.42 / 99 | 30,733 |
| 5 %  | ⚠️ near-full | 94.69 / 98 | 26,672 |
| 10 % | ⚠️ near-full | 94.42 / 99 | 30,564 |

**Reading.** Connectivity degrades gracefully: 94–97 % of peers at 2–10 % loss.
The strict convergence criterion is loss-sensitive, as expected.

### E4 — Latency / real-process emulation (tc netem, 4 real node processes)

| Scenario | Latency / loss | Health at end | Note |
|----------|----------------|---------------|------|
| Normal | 20 ms / 0 % | 4/4 | 3 peers/node, clean metrics |
| Mobile | 80 ms / 2 % | 3/4 | node-1 health timeout |
| Weak | 150 ms / 5 % | 3/4 | node-0 health timeout |
| Severe | 300 ms / 10 % | 0/4 | health checks time out; metrics still respond → nodes alive, health endpoint degraded |
| Partition | iptables split 30 s | 4/4 | loopback split not fully isolating (honest caveat, M4 fix) |
| Attack | peer flood 15 s | 4/4 | node-0 absorbed 3,012 flood packets / 1.5 MB, stayed healthy |

**Reading.** The degradation ladder 4/4 → 3/4 → 0/4 confirms netem genuinely
perturbs the real UDP traffic; the attack scenario shows a 250× packet burst
(≈12 → 3,012 packets) is absorbed without crash.

### E5 — Malicious peer injection (100 nodes, t=20 s)

| Metric | Value |
|--------|-------|
| Network converged post-attack | ✅ |
| Recovery time | 0.0 s |
| Avg peers post-attack | 98.61 / 99 |

### E6 — Partition and recovery (100 nodes, split at t=20 s)

| Metric | Value |
|--------|-------|
| Partitions healed after removal | ✅ |
| Convergence restored | ✅ |
| Recovery time | 0.0 s |
| Avg peers post-recovery | 98.09 / 99 |

### E7 — Gradient aging (ablation: aging disabled vs enabled)

Full half-life sweep (25–400 ms) is M2 work. The binary ablation was run:
disabling aging changed bytes by −0.4 % to +3.5 % across runs — within
run-to-run variance, indicating aging's cost is not measurable in this quiet
benchmark; its value is expected under stale-gradient regimes (funded-phase
sweep).

### E8 — Trust-score dynamics (ablation: trust scoring disabled vs enabled)

The trust engine's score paths are unit-tested (see §6 threat model and
`src/trust.rs` tests). Network-level ablation: disabling trust scoring changed
bytes by −6.2 % (run A) to +4.6 % (run B) — i.e. the raw overhead is within
run-to-run noise, while the security value (E5 containment) is real. This is a
documented trade-off, not a free lunch.

### E9 — Baselines (50 nodes, 60 s, seed 42; latest stable run)

| System | Converged | Conv. time (s) | Avg peers / max | Bytes sent (MB) | Δ bytes vs control |
|--------|-----------|----------------|-----------------|-----------------|--------------------|
| NWP (control: trust + aging + XOR-routing + lifecycle) | ✅ | 1.0 | 48.50 / 49 | 18.26 | — |
| No trust scoring | ✅ | 1.0 | 48.41 / 49 | 19.11 | +4.6 % |
| No gradient aging | ✅ | 1.0 | 48.43 / 49 | 18.91 | +3.5 % |
| No apoptosis | ✅ | 1.0 | 48.41 / 49 | 18.15 | −0.6 % |
| No neurogenesis | ✅ | 1.0 | 48.90 / 49 | 22.86 | **+25.2 %** |
| Random discovery (no XOR routing) | ✅ | 1.0 | 48.18 / 49 | 22.91 | **+25.4 %** |
| Static topology | ✅ | 1.0 | 48.30 / 49 | 19.10 | +4.6 % |

**Reading.** Two effects are **robust across runs**: removing neurogenesis
costs +23–25 % bytes and replacing XOR-closest routing with random discovery
costs +6–25 % bytes. The remaining ablations shift ±6 % run-to-run and are
treated as within variance (their value appears under churn/attack, not in the
quiet benchmark). The lifecycle system (birth/pruning) and the XOR distance
metric are the two largest efficiency levers measured so far.

---

## 9. Limitations

We state limitations without mitigation framing; each is a real constraint on the claims this paper can make.

1. **No wide-area deployment.** Every benchmark to date ran in simulation or on localhost UDP. Real UDP trials were limited to ~5–10 nodes before OS thread/socket limits on localhost intervened ([STATS.md §4](STATS.md)). Nothing here is evidence about behavior on the public Internet: NAT, asymmetric routing, congestion, and heterogeneous hardware are unmeasured.
2. **Simulator-to-reality gap.** The in-process simulator exercises the real protocol implementation but abstracts the network (loss, latency, and partitioning are injected as filters, not experienced as physics). The v4 hybrid results for very large $N$ additionally model nodes beyond 200 K statistically; we do not claim operational capacity at those scales.
3. **NAT traversal is incomplete.** STUN (RFC 5389) [7] client support exists, but symmetric-NAT hole punching and relay fallback are not implemented; nodes behind symmetric NATs cannot join. The deployment plan (Section 10) treats this as open engineering work, not a solved problem.
4. **No external audit.** The [audit report](docs/AUDIT_REPORT.md) and [security checklist](docs/SECURITY_CHECKLIST.md) are internal artifacts produced by the project's own tooling. There has been no independent security review, no formal verification of the Rust code beyond the type system, and no adversarial red-team exercise. Known residual gaps (no key zeroization, no traffic-analysis countermeasures, no in-process memory caps) remain open.
5. **Formal proofs rely on model assumptions.** The trust and rate-limit theorems in [docs/ALGORITHM_SPEC.md](docs/ALGORITHM_SPEC.md) and the formal model ([FORMAL_MODEL.md](FORMAL_MODEL.md)) assume the modeled event semantics, synchronous decay, and honest bookkeeping. Implementation drift, clock skew, and unforeseen interactions are not covered by the proofs.
6. **Sybil resistance is mitigation, not elimination.** Trust-based rate limiting raises the cost of Sybil attacks; it does not prevent identity fabrication, which is information-theoretically unavoidable without a trusted authority [4]. An attacker with sufficient distributed bandwidth can still degrade the network. Internal documentation also contains parameter inconsistencies (the Sybil threshold is 0.2 in code and the algorithm spec but 0.3 in one protocol section; bucket size is 8 in the wire spec and 20 in the research brief) that must be reconciled before external review.

---

## 10. Planned Deployment

The immediate plan is a controlled wide-area deployment, deliberately smaller and more honest than the eventual target:

- **20–50 cloud nodes** across at least two cloud providers and three regions, running the release binary in Docker (Alpine and scratch images, both with health checks; see [deploy/](deploy/)) to measure real convergence, bandwidth, and churn under actual Internet conditions. This is the prerequisite for retiring Limitation 1.
- **ARM and low-power hardware**: ARM64 builds and deployment on single-board computers to validate the constrained-environment design target (512 MB class devices, zero-copy FlatBuffer path, 1 ms tick budget).
- **NAT traversal completion**: symmetric-NAT hole punching and a minimal relay fallback, validated against real home routers, to close Limitation 3.
- **Security review**: external audit of the crypto and trust paths, remediation of the checklist's ❌/⚠️ items (key zeroization, traffic-analysis padding, memory caps), and reconciliation of the documentation inconsistencies noted in Section 9.
- **Independent reproduction**: the one-command pipeline ([scripts/reproduce.sh](scripts/reproduce.sh), [REPRODUCIBILITY.md](REPRODUCIBILITY.md)) exercised by reviewers on clean machines; all experiment configs and seeds are committed ([experiments/](experiments/)) before execution.
- **Public benchmark dashboard**: per-node Prometheus telemetry published on a live dashboard, with the same caveats this paper applies to simulated numbers — raw metrics, no cherry-picking, negative results included ([docs/RESEARCH_BRIEF.md](docs/RESEARCH_BRIEF.md)).

---

## 11. Conclusion

NWP is a concrete attempt at an open question: whether collaborative learning can operate without a coordinator, a registry, or a PKI, on infrastructure any reachable device can join. The contribution of this paper is the system description — a latency-weighted Kademlia DHT, an authenticated and encrypted UDP transport with three reliability tiers, trust-based rate limiting, Hebbian/STDP learning with aged gradient gossip, surprise-driven neurogenesis, and fitness-based apoptosis — together with a deterministic simulator and a committed nine-experiment evaluation matrix with defined metrics and placeholder results. We have deliberately not reported numbers that do not exist. The honest state of the project is: the protocol is implemented, unit-tested, and audited internally; its network-level claims are unproven outside simulation; and its security properties are mitigations, not theorems about the real world. The next step — a modest wide-area deployment with independent reproduction — is designed to replace speculation with measurement.

---

## References

1. Maymounkov, P., and Mazières, D. *Kademlia: A Peer-to-Peer Information System Based on the XOR Metric.* Proceedings of the 1st International Workshop on Peer-to-Peer Systems (IPTPS), 2002.
2. Bernstein, D. J. *Extending the Salsa20 Nonce.* Submission to the eSTREAM project, 2011. https://cr.yp.to/snuffle/xsalsa-20081128.pdf
3. Bernstein, D. J., Duif, N., Lange, T., Schwabe, P., and Yang, B.-Y. *High-Speed High-Security Signatures.* Journal of Cryptographic Engineering 2(2):77–89, 2012.
4. Josefsson, S., and Liusvaara, I. *Edwards-Curve Digital Signature Algorithm (EdDSA).* RFC 8032, IETF, 2017.
5. Nir, Y., and Langley, A. *ChaCha20 and Poly1305 for IETF Protocols.* RFC 8439, IETF, 2018.
6. Douceur, J. R. *The Sybil Attack.* Proceedings of the 1st International Workshop on Peer-to-Peer Systems (IPTPS), 2002.
7. Rosenberg, J., Mahy, R., Matthews, P., and Wing, D. *Session Traversal Utilities for NAT (STUN).* RFC 5389, IETF, 2008.
8. Singh, A., Ngan, T.-W., Druschel, P., and Wallach, D. S. *Defending Against Eclipse Attacks in Peer-to-Peer Networks.* Proceedings of the 1st ACM SIGOPS EuroSys Conference, 2006.
9. McMahan, H. B., Moore, E., Ramage, D., Hampson, S., and Agüera y Arcas, B. *Communication-Efficient Learning of Deep Networks from Decentralized Data.* Proceedings of the 20th International Conference on Artificial Intelligence and Statistics (AISTATS), 2017. (FedAvg.)
10. Lian, X., Zhang, C., Zhang, H., Hsieh, C.-J., Zhang, W., and Liu, J. *Can Decentralized Algorithms Outperform Centralized Algorithms? A Case Study for Decentralized Parallel Stochastic Gradient Descent.* Advances in Neural Information Processing Systems 30 (NeurIPS), 2017.
11. Bi, G.-q., and Poo, M.-m. *Synaptic Modifications in Cultured Hippocampal Neurons: Dependence on Spike Timing, Synaptic Strength, and Postsynaptic Cell Type.* Journal of Neuroscience 18(24):10464–10472, 1998.
12. Langley, A., Hamburg, M., and Turner, S. *Elliptic Curves for Security.* RFC 7748, IETF, 2016.
13. Google. *FlatBuffers: Efficient Cross-Platform Serialization Library.* https://flatbuffers.dev

---

*Project documentation referenced inline: [ARCHITECTURE](docs/ARCHITECTURE.md) · [ALGORITHM_SPEC](docs/ALGORITHM_SPEC.md) · [WIRE_PROTOCOL](docs/WIRE_PROTOCOL.md) · [SECURITY_CHECKLIST](docs/SECURITY_CHECKLIST.md) · [AUDIT_REPORT](docs/AUDIT_REPORT.md) · [RESEARCH_BRIEF](docs/RESEARCH_BRIEF.md) · [STATS](STATS.md) · [GRANT](GRANT.md) · [REPRODUCIBILITY](REPRODUCIBILITY.md) · [FORMAL_MODEL](FORMAL_MODEL.md) · [experiments/](experiments/)*
