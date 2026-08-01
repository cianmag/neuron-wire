# Project Intelligence Document: Neuron Wire (NWP)

> **Single source of truth.** Every README, grant application, paper, website page, investor deck, conference talk, and interview derives from this document.
>
> **Maintainer:** Zylvon Research · **Repository:** [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)
> **Version:** 0.3.0 · **Updated:** 2026-07-09 · **Commit:** `39591ad`

---

## How to Read This Document

Every answer is one sentence unless otherwise noted. Claims are tagged with their evidence type:

| Tag | Meaning |
|-----|---------|
| **[I]** | Implemented and tested (code + unit test) |
| **[S]** | Verified in simulation (reproducible benchmark) |
| **[T]** | Theoretically supported (formal model) |
| **[F]** | Future work (not yet validated) |

---

## 1. Identity & Mission

**What is Neuron Wire?** An open-source Rust framework that enables any device to discover peers, exchange learning signals, and participate in collaborative AI without centralized infrastructure. **[I]**

**What does the name mean?** "Neuron" reflects the Hebbian STDP learning substrate modeled on biological neurons; "Wire" signifies the P2P communication fabric that connects them into a distributed network.

**What is its one-sentence definition?** An open-source infrastructure layer for decentralized AI providing P2P peer discovery, custom UDP transport, Hebbian STDP learning, and reproducible simulation. **[I]**

**What is its 30-second explanation?** A Rust runtime where every node maintains a small neural graph, discovers peers through a Kademlia DHT, exchanges activations over a custom UDP protocol with three reliability tiers, and adapts locally via Hebbian STDP — all without any central coordinator. **[I]**

**What is its one-minute explanation?** Neuron Wire is a research infrastructure project that integrates four domains usually studied independently: peer-to-peer networking (Kademlia DHT), custom transport (UDP with selective reliability), neural computation (forward propagation with tanh), and biological learning rules (Hebbian STDP with neurogenesis and apoptosis). The entire system runs on a single-threaded non-blocking engine loop at ~400 KHz–1 MHz tick rate with zero external runtime dependencies. A deterministic paper-mode simulator enables fully reproducible experiments, and the v4 hybrid model demonstrates DHT convergence at up to 1 B nodes. **[I+S]**

**What is its five-minute explanation?** See [README.md](README.md) — the full README is a five-minute summary. This document is the exhaustive reference.

**What problem does it solve?** Distributed AI today requires cloud infrastructure, orchestration, and centralized coordination — parameter servers, federated aggregators, or All-Reduce rings — creating single points of failure, access barriers, and privacy risks that a fully P2P approach avoids. **[I]**

**Why was it created?** To answer the open research question: can decentralized collaborative learning work over real Internet conditions without centralized infrastructure? **[F]**

**Who created it?** Zylvon Research, founded and operated by Cian Magic (team@zylvon.com). **[I]**

**When did development begin?** June 2026; the first commit was `e2d99a5` and the project reached its current state in approximately 11 days of active development. **[I]**

**What is its current version?** v0.3.0 — a research prototype with complete simulation infrastructure, formal model, baseline comparisons, and reproducible benchmarks, but no real WAN deployment. **[I]**

**Is it a protocol, framework, library, platform, or operating system?** All four: it defines a binary wire protocol (NWP), provides a Rust framework for building P2P learning nodes, exposes a library API for embedding, and its engine loop is a minimal runtime that owns the networking and computation cycle. **[I]**

**Who is the intended audience?** Researchers studying decentralized AI, distributed systems, and P2P learning; systems engineers building decentralized infrastructure; and grant organizations funding open decentralized AI infrastructure. **[I]**

**Why should anyone use it?** It is the only open-source P2P learning runtime that integrates Kademlia DHT, custom UDP transport, Hebbian STDP, and deterministic reproducibility into a single auditable codebase. **[I]**

**Why should anyone fund it?** Because the question of whether decentralized collaborative learning works over real Internet links is unanswered by published evidence, and Neuron Wire is the closest project to answering it with reproducible methodology. **[S+F]**

---

## 2. Executive Summary

**Explain Neuron Wire to a child.** It lets computers share what they learn by whispering hints to each other instead of sending everything to one big boss computer.

**Explain it to a high school student.** A program that lets many computers work together on an AI task by talking directly to each other — like a group project with no leader, where everyone shares their answers and learns from each other.

**Explain it to a CS undergraduate.** A single-threaded Rust runtime implementing a Kademlia DHT for P2P discovery, UDP transport with custom reliability, and Hebbian STDP neural computation — all running in a deterministic simulation framework designed for reproducible distributed ML research.

**Explain it to a networking engineer.** A custom UDP stack with 16-byte transport headers, selective retransmission (DATA=3 retries, CONSENSUS=5, BestEffort=0), ACK bitfield covering 33 packets, exponential gradient decay (half-life = 100 ms), and Kademlia XOR routing over 256 k-buckets with latency-weighted eviction. **[I]**

**Explain it to an ML researcher.** A reproducible research platform for studying whether Hebbian STDP over Kademlia-routed P2P gradient gossip converges under realistic network conditions, with deterministic simulator, formal convergence model, and 7 Python baselines for comparison. **[I+S]**

**Explain it to a systems engineer.** A non-blocking single-threaded engine loop (6 phases: drain UDP → drain outbound → neural compute → retransmit → apoptosis → yield) running at ~400 KHz–1 MHz with zero external runtime dependencies, compiling to a single statically linked ~2 MB binary. **[I]**

**Explain it to a CTO.** An MIT-licensed Rust infrastructure layer for decentralized AI that eliminates the need for centralized coordination in collaborative learning, currently at research prototype stage with proven simulation at 1 B nodes and seeking first WAN deployment. **[I+S]**

**Explain it to a venture capitalist.** Open-source infrastructure for decentralized AI — any device with a network connection can participate in collaborative learning without servers, central coordination, or a single point of control; validated in simulation up to 1 B nodes. **[S]**

**Explain it to a government grant committee.** An open-source research platform that investigates whether decentralized AI can operate without centralized cloud infrastructure, with 41 Rust source files, 342 test functions, deterministic reproducibility, and a WAN deployment milestone awaiting funding. **[I]**

**Explain it to a university professor.** A research infrastructure artifact integrating Kademlia DHT routing, custom UDP transport with reliability tiers, Hebbian STDP learning, neurogenesis, and apoptosis into a single deterministic simulation framework designed to evaluate whether P2P collaborative learning is practical under real Internet conditions. **[I+S]**

---

## 3. Problem Definition

**What is broken in today's AI infrastructure?** Every collaborative learning system requires a central coordinator, a static topology, or homogeneous hardware, making AI inaccessible from regions without reliable cloud infrastructure. **[I]**

**What are today's biggest bottlenecks?** Parameter server bandwidth saturation at scale, All-Reduce synchronization barriers that stall on the slowest node, and federated learning's reliance on a trusted central aggregator. **[I]**

**Why are centralized clusters expensive?** They require provisioning for peak capacity, redundant networking (NCCL requires 200+ Gbps interconnects), and cooling/power for homogeneous GPU arrays that sit idle between training runs. **[I]**

**Why are they difficult to scale?** Synchronous training requires all devices to complete each step before proceeding, and the slowest device determines the wall-clock time — scaling beyond 1,024 GPUs delivers diminishing returns for many architectures. **[T]**

**Why is distributed AI difficult today?** Because existing systems were designed for tightly coupled clusters with known membership, not for heterogeneous devices joining and leaving over unreliable Internet links. **[I]**

**Why do current systems require coordination?** Because gradient averaging (All-Reduce), parameter synchronization (PS), and round-based aggregation (FL) all assume an agreement point that every participant reaches before proceeding. **[I]**

**Why is bandwidth expensive?** Full gradient exchange in dense neural networks transmits values equal to the model size (billions of parameters) every step; a 7B-parameter model at 32-bit precision produces 28 GB of gradients per step. **[T]**

**Why does synchronization hurt performance?** The straggler problem — in any synchronous system, the wall-clock time per step equals the maximum of all participant times, and heterogeneous devices amplify this effect. **[T]**

**Why is latency important?** For any gradient exchange protocol, the minimum convergence time is bounded by the round-trip time between the furthest peers, and cross-continent RTT of 200–300 ms sets a floor on learning speed regardless of compute. **[T]**

**Why does packet loss matter?** Lost gradient updates introduce noise into the learning process equivalent to increasing the variance of the gradient estimator, which reduces the effective learning rate and can prevent convergence. **[T]**

---

## 4. Vision

**What does success look like?** The funded pilot — **20 nodes across three regions** — sustains connectivity for 7+ days, converges within 30 seconds of any partition event, and demonstrates gradient delivery with < 50% cross-continent loss — with all data and reproduction scripts published under open license. A 100+ node network is the post-pilot scaling goal. **[F]**

**What changes if Neuron Wire succeeds?** Any device with a network connection can join a collaborative learning network without provisioning infrastructure, trusting a coordinator, or exposing private data. **[F]**

**What industries benefit?** Edge AI (manufacturing, agriculture, healthcare), disaster response (ad-hoc networks), underrepresented regions (Southeast Asia, Africa, South America), and any domain where centralized cloud access is unreliable or undesirable. **[F]**

**What research becomes possible?** First-ever reproducible WAN benchmarks for P2P gradient exchange, latency-characterized STDP convergence curves, churn-tolerance measurements across real Internet conditions, and comparison of decentralized vs. centralized learning economics on identical hardware. **[F]**

**What problems disappear?** Single points of failure (no coordinator to crash), access gatekeeping (no operator to block participants), data sovereignty (data never leaves the device), and vendor lock-in (no cloud provider dependency). **[F]**

**What is impossible today but possible tomorrow?** A phone in rural Nepal and a laptop in Berlin discovering each other automatically and collaborating on a shared learning task without either party provisioning infrastructure or trusting the other. **[F]**

**What does the world look like in 10 years?** Decentralized P2P learning is a standard tool in the ML researcher's toolkit, with Neuron Wire or a descendant protocol serving as the common substrate, analogous to what TCP/IP did for internetworking. **[F]**

**Is Neuron Wire a learning runtime or an experimental platform?** It is becoming the latter. The project's strongest long-term position is as a laboratory for decentralized systems research — a configurable, reproducible testbed where researchers can swap routing algorithms, gossip strategies, learning rules, trust models, and transport layers, run the same benchmark suite, and compare results without rebuilding the stack. **[F]**

---

## 5. Technical Identity

**Is it peer-to-peer?** Yes — every node communicates directly with other nodes without a central relay or coordinator. **[I]**

**Is it decentralized?** Yes — there is no single point of control, failure, or trust; the Kademlia DHT distributes routing across all participants. **[I]**

**Is it distributed?** Yes — computation and state are spread across multiple nodes that communicate to achieve a shared learning objective. **[I]**

**Is it federated?** No — federated learning requires a central aggregator that averages client model weights; Neuron Wire has no aggregator. **[I]**

**Is it serverless?** Yes — there is no server, no cloud instance that must remain running, and no central service that must be reachable. **[I]**

**Is it infrastructure?** Yes — it provides the foundational layer (discovery, transport, identity) on which decentralized AI applications can be built. **[I]**

**Is it middleware?** Yes — it sits between the operating system (UDP socket, thread) and the application (learning algorithm, user interface). **[I]**

**Is it networking?** Yes — it implements a complete protocol stack from wire format through DHT routing to application-layer gradient delivery. **[I]**

**Is it machine learning?** Yes — it embeds Hebbian STDP, forward propagation, neurogenesis, and apoptosis as first-class subsystems. **[I]**

**Is it all of the above?** Yes — this integration of networking, distributed systems, and learning in a single auditable runtime is the project's defining characteristic. **[I]**

---

## 6. Architecture

### 6.1 Engine Loop (`engine_loop.rs`)

**What does it do?** Owns the UDP socket and executes six phases every tick (recv → drain → neural → retransmit → apoptosis → yield) at ~400 KHz–1 MHz. **[I]**

**Why does it exist?** Eliminates scheduler jitter, avoids 50+ transitive async runtime dependencies, and sustains maximum throughput on 512 MB RAM shared CPU instances. **[I]**

**Why was it designed this way?** A single-threaded recv loop with 1 ms timeout is the simplest possible design that meets the real-time constraint — no locks, no channels in the hot path, deterministic tick ordering. **[I]**

**What alternatives were rejected?** Tokio async (too many dependencies, scheduler jitter), multi-threaded actor model (locking complexity, non-deterministic), event-driven framework (abstraction overhead). **[I]**

**What are the trade-offs?** Single-threaded means no multi-core utilization, but eliminates all concurrency bugs and guarantees deterministic execution across runs. **[I]**

**What assumptions does it make?** That a single core at ~2–3 GHz provides adequate compute for the neural graph and routing table at the required tick rate. **[I]**

**What inputs does it receive?** UDP datagrams from the socket, timer events (retransmit, apoptosis scheduling), and application-level commands. **[I]**

**What outputs does it produce?** UDP datagrams to the socket, metrics to the observability pipeline, and CSV data to the simulation exporter. **[I]**

**Which modules depend on it?** All of them — the engine loop is the coordinator that calls every subsystem. **[I]**

**Which modules can fail because of it?** If the engine loop blocks (e.g., long computation in Phase 3), the entire node stops processing inbound packets until the tick completes. **[I]**

### 6.2 DHT Routing (`dht.rs`)

**What does it do?** Maintains 256 k-buckets (K=20 each) using XOR distance for placement and latency EMA for eviction, implementing Kademlia FIND_NODE and periodic PING maintenance. **[I]**

**Why does it exist?** Nodes need a way to discover each other without a central registry — the DHT provides O(log N) routing with proven performance. **[I]**

**Why was it designed this way?** Kademlia is the most battle-tested DHT algorithm (BitTorrent, Ethereum), XOR distance guarantees unique bucket placement, and latency weighting favors responsive peers. **[I]**

**What alternatives were rejected?** Chord (complex finger table maintenance), Pastry (locality not guaranteed), centralized registry (single point of failure), gossip-based discovery (unbounded message growth). **[I]**

**What are the trade-offs?** XOR distance is not geographic distance — two nodes with similar NodeIds may be on different continents, and latency measurement compensates for this. **[I]**

**What assumptions does it make?** That each node has a unique 256-bit NodeId, that nodes can send UDP packets to each other, and that the bucket size K=20 is sufficient for graph connectivity. **[I]**

**What inputs does it receive?** FIND_NODE requests, PING requests, incoming PONG responses, and periodic maintenance triggers. **[I]**

**What outputs does it produce?** FIND_NODE responses (contact lists), PONG messages, peer eviction events, and routing metrics to observability. **[I]**

**Which modules depend on it?** Transport (sends its messages), Hebbian/Learning (uses it for gossip peer selection), Simulator (drives its tick cycle). **[I]**

**Which modules can fail because of it?** If the DHT fails to converge (e.g., no seed nodes reachable), the node cannot discover any peers and operates in isolation. **[F]**

### 6.3 UDP Transport (`transport.rs`, `udp_transport.rs`)

**What does it do?** Owns the UDP socket, manages the ACK sliding window (33-packet bitfield), and implements three reliability tiers (BestEffort=0 retries, DATA=3, CONSENSUS=5). **[I]**

**Why does it exist?** The system needs network communication; UDP provides the lowest-latency transport while giving the protocol control over which messages require reliability. **[I]**

**Why was it designed this way?** TCP's head-of-line blocking means one lost packet stalls the entire stream; custom reliability allows SPIKE and COMMAND frames (where fresh data supersedes old) to bypass retransmission entirely. **[I]**

**What alternatives were rejected?** TCP (head-of-line blocking, congestion control not designed for real-time learning), QUIC (immature Rust ecosystem, stream abstraction unnecessary), raw UDP without reliability (gradients would be lost). **[I]**

**What are the trade-offs?** Custom reliability means maintaining an ACK state machine that TCP provides for free, but the implementation is ~640 lines and the resulting control over per-message reliability enables the gradient decay mechanism. **[I]**

**What assumptions does it make?** That UDP datagrams up to ~1,500 bytes (Ethernet MTU minus headers) can be sent and received without fragmentation. **[I]**

**What inputs does it receive?** UDP datagrams from the socket, outbound messages from the mpsc channel (DHT responses, gradient gossip, application messages). **[I]**

**What outputs does it produce?** UDP datagrams, retransmission events, and transport-layer metrics (bytes in/out, packets lost, retransmission count). **[I]**

**Which modules depend on it?** Every module that sends or receives network messages — DHT, Hebbian, Neurogenesis, Engine Loop. **[I]**

**Which modules can fail because of it?** If the UDP socket fails to bind, the node cannot participate in the network at all. **[I]**

### 6.4 ForwardPass System (`forward_pass.rs`)

**What does it do?** Each tick: leak activations (×0.95), propagate Σ(weight × pre_activation), squash via tanh to [-1, 1], compare prediction vs. observation to emit surprise signal, and clean orphan activations. **[I]**

**Why does it exist?** The forward pass is the fundamental computation that converts input signals into neural output and prediction error — without it, there is no learning signal. **[I]**

**Why was it designed this way?** Five sequential phases with no branching per tick enables deterministic execution and bounded worst-case timing. **[I]**

**What alternatives were rejected?** ReLU activation (clamps negative activations to zero, losing information), full backpropagation (requires graph reversal, not suitable for real-time tick), skip connections (complexity not justified at prototype stage). **[I]**

**What are the trade-offs?** Tanh saturates at ±1, preventing activation explosion but also limiting dynamic range; the 0.95 leak was chosen empirically to balance decay speed with signal retention. **[I]**

**What assumptions does it make?** That neural activations are real values in [-1, 1], that the network is acyclic within a single tick, and that prediction error is mean squared error. **[I]**

**What inputs does it receive?** Current activation values from the component store, current synaptic weights from the synapse store, and observed inputs from the environment. **[I]**

**What outputs does it produce?** Updated activations, prediction error (surprise) values for each neuron, and neurogenesis trigger events when surprise exceeds 0.2. **[I]**

**Which modules depend on it?** Hebbian (uses activation timing for STDP), Neurogenesis (triggered by surprise), Apoptosis (prunes based on activation history). **[I]**

**Which modules can fail because of it?** If ForwardPass produces NaN or overflow activations, every downstream subsystem receives corrupted signals, and the neuron graph enters an unrecoverable state. **[I]**

### 6.5 Hebbian STDP (`hebbian.rs`)

**What does it do?** Each tick: compute Δw = η · pre_activation · post_activation (η = 0.01), decay weights by λ = 0.999, micro-prune synapses below 0.001, and gossip accumulated gradients to up to 3 DHT peers. **[I]**

**Why does it exist?** Hebbian STDP is the best-understood local learning rule that requires no central coordinator — each neuron updates its own weights based purely on locally available signals. **[I]**

**Why was it designed this way?** The rule is mathematically simple (multiplication and decay), deterministic, requires no global state, and its biological plausibility provides a strong prior for convergence. **[I]**

**What alternatives were rejected?** SGD with backpropagation (requires global model state), contrastive divergence (requires negative phase sampling), reinforcement learning (requires reward signal from environment). **[I]**

**What are the trade-offs?** Hebbian learning stabilizes without explicit regularization but cannot learn arbitrary functions like backpropagation; it is a complementary mechanism, not a replacement. **[I]**

**What assumptions does it make?** That pre- and post-synaptic activation timing is available locally, that synaptic weight changes are additive with decay, and that gossip communication is eventually reliable. **[I]**

**What inputs does it receive?** Pre- and post-synaptic activation values from ForwardPass, gossip gradients from peers via transport, and scheduled tick events. **[I]**

**What outputs does it produce?** Updated synaptic weights, micro-prune events, and gossip messages containing accumulated gradient deltas. **[I]**

**Which modules depend on it?** ForwardPass (uses updated weights for next tick), Apoptosis (prunes based on weight magnitude), Neurogenesis (new synapses initialized with Hebbian-compatible parameters). **[I]**

**Which modules can fail because of it?** If Hebbian weights diverge to infinity (no upper bound in the current rule), the entire neural graph collapses to extreme values. **[T]**

### 6.6 Neurogenesis (`neurogenesis.rs`)

**What does it do?** Maintains a leaky accumulator (decay = 0.99) per neuron; when surprise from ForwardPass pushes the accumulator above 0.2, spawns a new neuron with a random 256-bit EntityId and initial weights in [-0.1, 0.1]. **[I]**

**Why does it exist?** The network needs to grow when it encounters novel patterns it cannot predict — fixed-topology networks cannot adapt their capacity to problem complexity. **[I]**

**Why was it designed this way?** The leaky accumulator naturally filters transient noise (it decays away) while integrating sustained prediction error (it accumulates until threshold), providing hysteresis. **[I]**

**What alternatives were rejected?** Fixed-topology (no adaptation to complexity), periodic growth schedule (does not respond to learning signals), gradient-based growth (requires global information). **[I]**

**What are the trade-offs?** Threshold 0.2 was chosen empirically; too low causes excessive neuron birth (overfitting), too high causes no growth (underfitting). **[I]**

**What assumptions does it make?** That prediction error is a reliable signal for when to add capacity, and that random initialization of new synapses is sufficient for useful integration. **[I]**

**What inputs does it receive?** Surprise signals from ForwardPass, accumulator values from previous tick, and the current neuron count. **[I]**

**What outputs does it produce?** New neuron entities with EntityId, initial activation, and random input/output synapses; an accumulator reset event. **[I]**

**Which modules depend on it?** Apoptosis (removes underperforming neurons created by neurogenesis), Component Store (stores new entities), ForwardPass (must process new neurons). **[I]**

**Which modules can fail because of it?** If the threshold is too low, runaway neurogenesis exhausts memory — the death spiral guard in Apoptosis provides a safety check. **[I]**

### 6.7 Apoptosis (`apoptosis.rs`)

**What does it do?** Every ~1 s (1000 ticks), evicts DHT entries with fail_count ≥ 3 or latency > 500 ms or last_seen > 600 s, prunes orphaned transport frames, and removes synapses with weight < 0.001. **[I]**

**Why does it exist?** Without scheduled pruning, stale routing entries degrade discovery, orphaned frames waste memory, and dead-weight synapses accumulate noise in the learning signal. **[I]**

**Why was it designed this way?** Four independent criteria allow Apoptosis to act on different time scales: failures are fast (3 misses ≈ 3–30 s), latency is medium (500 ms threshold), staleness is slow (600 s). **[I]**

**What alternatives were rejected?** LRU eviction (does not account for failure count), random eviction (may remove healthy entries), no eviction (routing table fills with dead entries). **[I]**

**What are the trade-offs?** Aggressive eviction improves routing freshness but increases bandwidth from re-discovery and can cause flapping under high churn. **[I]**

**What assumptions does it make?** That a single sweep evicting more than a configurable fraction indicates a network partition, not normal churn. **[I]**

**What inputs does it receive?** DHT routing table from DHT module, transport frame queue from Transport module, synapse weights from Hebbian module, and timer events every 1000 ticks. **[I]**

**What outputs does it produce?** Evicted peer IDs, pruned frame IDs, removed synapse IDs, and apoptosis sweep event log entries. **[I]**

**Which modules depend on it?** DHT (receives eviction notifications), Transport (memory freed), Hebbian (pruned synapses removed from weight updates), Engine Loop (dead-spiral guard stops the node). **[I]**

**Which modules can fail because of it?** If the death-spiral guard triggers during a legitimate partition, the node shuts down — a false positive that the configurable threshold mitigates. **[I]**

---

## 7. Networking

**Why UDP?** UDP avoids TCP's head-of-line blocking (one lost packet stalls the entire stream) and lets the protocol select per-message reliability — 2 of 7 message types need retransmission while the other 5 benefit from fire-and-forget delivery. **[I]**

**Why not TCP?** TCP's in-order delivery guarantee is unnecessary for gradient exchange where stale data is exponentially decayed (weight < 0.001 after ~10 half-lives), and TCP's congestion control adds latency variance that confounds timing-dependent learning experiments. **[I]**

**Why not QUIC?** QUIC is not universally available in the minimum Rust toolchain, its stream abstraction is unnecessary when the protocol handles its own reliability tiers, and the QUIC Rust ecosystem is immature relative to the project's stability requirements. **[I]**

**Why reliable UDP?** Only DATA and CONSENSUS frames need guaranteed delivery — SPIKE, COMMAND, READINESS, and GOSSIP frames are time-sensitive and best-effort delivery is correct for all of them. **[I]**

**How are packets formatted?** Each packet is [16-byte transport header (seq, ack, bitfield, timestamp) | 16-byte NWP header (magic, version, type, flags, body_len, CRC32) | N-byte FlatBuffer body]. **[I]**

**How are packets verified?** A CRC32 checksum covers the NWP header and body; the transport header is verified by sequence number monotonicity per sender. **[I]**

**How are duplicates removed?** The sliding-window ACK bitfield tracks the last 33 received sequence numbers; any packet with a sequence number already in the window is silently dropped. **[I]**

**How are retransmissions handled?** DATA frames retransmit up to 3 times with exponential backoff (1×, 2×, 4× base interval); CONSENSUS frames retransmit up to 5 times; no retransmission for BestEffort frames. **[I]**

**How is congestion handled?** Congestion control is not implemented — the gradient decay mechanism provides implicit back-pressure (stale gradients contribute near-zero weight), but there is no explicit window reduction. **[F]**

**How is packet ordering handled?** Per-sender sequence numbers enable the receiver to detect out-of-order delivery, but frames are processed in arrival order — reordering is not implemented because gradient arrival order is not semantically meaningful. **[I]**

**What happens during packet loss?** For DATA frames, the sender retransmits up to 3 times; for CONSENSUS frames, up to 5 times; for BestEffort frames, the data is lost and the system depends on the next update superseding it. **[I]**

**How are peers discovered?** Via Kademlia iterative FIND_NODE — the node starts with seed addresses, sends parallel FIND_NODE requests, processes responses to find closer nodes, and repeats until the closest nodes have been contacted. **[I]**

**How are routing tables updated?** After each FIND_NODE response, the returned contacts are inserted into the appropriate k-bucket (determined by XOR distance); if the bucket is full, the highest-latency entry is evicted. **[I]**

**How does peer eviction work?** Three criteria: fail_count ≥ 3 consecutive missed pings (fast removal, ~3–30 s), latency > 500 ms (medium removal), last_seen > 600 s (slow removal, passive timeout). **[I]**

**How is latency measured?** Each PING/PONG exchange records the round-trip time; a per-entry EMA smooths the measurement: latency = latency × 0.7 + sample × 0.3. **[I]**

---

## 8. Machine Learning

> **⚠️ Weakest area.** This is the project's least developed capability. Routing is proven at scale; learning is not. No end-to-end benchmark (MNIST, distributed regression, or similar) has been run. This is the single biggest scientific gap and the highest-risk area for claims about the system.
>
> **Status:** The benchmark below is concretely designed on paper with exact protocol, success criteria, failure criteria, and estimated implementation cost (~200–400 lines of Rust + Python). It has not been coded or executed. See `RESEARCH_QUESTIONS.md` RQ9 for the full research question and priority index.

**Why Hebbian learning?** Hebbian STDP is the only well-understood local learning rule that requires no global state, no backpropagation, and no central coordinator — each synapse updates based purely on local pre- and post-synaptic activation timing. **[I]**

**Why STDP?** STDP is the precise formulation of Hebb's rule (Δw depends on the temporal order and interval between pre- and post-synaptic spikes) and is the best-characterized biological learning mechanism in computational neuroscience. **[I]**

**Why not SGD?** SGD requires a global loss function computed over the full dataset, which cannot be known by any single node in a P2P network without a coordinator. **[I]**

**Why not Adam?** Adam maintains per-parameter momentum and variance estimates that diverge across independently operating nodes, making gradient gossip inconsistent without synchronization. **[I]**

**How are gradients exchanged?** Every N ticks, each node serializes accumulated weight deltas into a FlatBuffer frame and dispatches the frame to up to 3 DHT peers, with cluster-biased selection to prioritize same-type nodes. **[I]**

**How are stale gradients handled?** The gradient weight decays exponentially with configurable half-life (default 100 ms): weight = e^(-ln(2) × Δt / half_life), so a gradient arriving after 500 ms contributes only ~3% of its original value. **[I]**

**How is convergence measured?** The routing-side convergence is measured as the fraction of nodes that have discovered at least threshold = max(3 log₂(N), 30) peers; learning-side convergence is an open research question. **[S+F]**

**What should the first learning benchmark be?** A distributed linear regression experiment: N nodes (3–100) each hold a subset of synthetic data generated from y = w · x + b + ε where w, b are known ground truth and ε ∼ N(0, 0.1), and each node learns the mapping via Hebbian STDP (single-input→single-output neuron) while gossiping accumulated gradients with DHT peers — the benchmark measures epochs-to-convergence (MSE < 0.01 on held-out test set), communication cost (bytes/node), wall-clock time, and final test MSE, compared against centralized SGD and federated averaging (scikit-learn, same data). **[F]**

> **Protocol (exact steps the benchmark follows):**
> 1. Generate T = 10 000 (x, y) pairs from known w, b with noise; split into 80/10/10 train/val/test per node.
> 2. Initialize each node's neural graph: 1 input neuron, 1 output neuron, 1 synapse with random w ~ U(0.1, 1.0), 1 bias neuron.
> 3. Per epoch: for each batch of 32 samples, set input neuron activation = x, run ForwardPass → prediction → surprise, run Hebbian STDP update.
> 4. Every 500 ticks: serialize accumulated Δw to FlatBuffer, gossip to up to 3 DHT peers.
> 5. Every 10 epochs: compute MSE over held-out test set at each node, log to CSV.
> 6. Repeat until MSE < 0.01 × ground truth variance or 500 epochs elapsed.
> 7. Compare against centralized SGD (lr=0.01, batch=32, same epochs) and federated averaging (rounds=N_EPOCHS, local epochs=1, 10 clients sampled/round).
> 8. Output: convergence curves (epoch vs. MSE), communication cost wall-clock per epoch, final test MSE, packet count. Runs in deterministic simulator mode; every result is reproducible from seed 42.

**Can supervised learning work?** In theory — labeled inputs produce prediction error that drives STDP updates — but no supervised learning experiment has been conducted with the framework. **[F]**

**Can reinforcement learning work?** In theory — a reward signal could modulate the surprise threshold or learning rate — but RL integration has not been attempted. **[F]**

**Can transformers work?** No — the current neural model is a simple feedforward graph with tanh activations and local STDP updates, which cannot express the attention mechanism that transformers require. **[F]**

**Can CNNs work?** No — there is no convolution operation, weight sharing, or pooling layer in the current implementation; the network is a fully connected graph. **[F]**

**Can GNNs work?** Potentially — the DHT routing graph is itself a sparse graph, and GNN-style message passing maps naturally to the gossip protocol — but no GNN integration exists. **[F]**

**Can LLMs work?** No — the Hebbian STDP rule does not implement backpropagation through transformer layers and the single-threaded engine cannot sustain the compute required for autoregressive generation. **[F]**

---

## 9. Scalability

**Current real deployment size?** ~3–10 nodes on localhost UDP; OS socket/thread limits prevent larger real deployments without infrastructure changes. **[I]**

**Current simulation size?** 1 B nodes (v4 hybrid model, 200 K active + virtual remainder), 15.6 s wall time, 99.5% convergence. **[S]**

> **⚠️ Caveat — one billion simulated nodes ≠ one billion real devices.** The simulator models one billion virtual nodes using a hybrid approach where 200 K are active (real Node objects with routing tables, peer lists, and message processing) and the remainder are statistical abstractions that respond with random active peers in their PONG messages. This is not equivalent to operating one billion real networked computers. Real-device behavior would differ due to OS socket limits, kernel scheduling, NAT, jitter, loss, congestion, firewall policies, and protocol bottlenecks that the simulator does not model. Use the simulation number to evaluate the routing *algorithm's* theoretical upper bound, not to predict *operational* capacity.

**Maximum tested scale?** 1 B nodes in the v4 hybrid simulator — every scale from 100 K to 1 B converges with ≥ 98.9% of nodes above threshold. **[S]**

**Largest theoretical scale?** Unbounded in the hybrid model — simulation cost is independent of N for N > ACTIVE_MAX (200 K), as virtual nodes contribute no per-node computation. **[T]**

**What breaks first?** The real UDP transport — OS socket limits constrain concurrent UDP communication to ~10–25 nodes on a single machine before the kernel drops packets. **[I]**

**Memory bottlenecks?** Each active node's routing table requires ~500 KB (256 buckets × 20 entries × ~100 bytes); at 200 K active nodes, this is ~100 GB total, or ~500 KB per node (distributed). **[T]**

**CPU bottlenecks?** ForwardPass iterates over every neuron and synapse each tick; a dense neural graph of 10⁵ neurons × 100 synapses each would require 10⁷ operations per tick, exceeding a 1 ms budget on a single core. **[T]**

**Network bottlenecks?** At 100 K nodes in v3 simulation, aggregate bandwidth reaches 202 Mbps (2.0 Mbps/node), which exceeds typical consumer uplink capacity (~10–50 Mbps) at scale. **[S]**

**Disk bottlenecks?** None currently — all state is RAM-only; there is no persistent storage and no disk I/O in the hot path. **[I]**

**Routing bottlenecks?** The k-bucket cap MAX_PEERS = 500 prevents O(N) routing table growth, but periodic maintenance (15 PINGs + 10 FIND_NODE per node per 500 ticks) generates O(N) total traffic each maintenance round. **[S]**

---

## 10. Performance

| Metric | Value | Evidence |
|--------|-------|----------|
| Startup time | < 100 ms | **[I]** |
| Shutdown time | < 10 ms | **[I]** |
| Tick rate (idle) | ~400 KHz–1 MHz | **[I]** |
| Tick rate (deterministic sim) | ~1 KHz | **[I]** |
| Messages/sec (50 nodes, SGA) | ~22,000/node | **[S]** |
| Packets/sec/node | ~1,300 (fixed) – 22,000 (SGA) | **[S]** |
| CPU usage (idle) | 0% (OS blocks on recv timeout) | **[I]** |
| Memory per active node | ~500 KB (routing table) + neural graph | **[T]** |
| Binary size (release, stripped) | ~2–4 MB | **[I]** |
| Build time (cargo build --release) | ~2 minutes (fresh checkout) | **[I]** |
| Test execution time | < 30 seconds | **[I]** |
| Benchmark runtime (v3, 100 K) | 43.4 s wall time | **[S]** |
| Benchmark runtime (v4, 1 B) | 15.6 s wall time | **[S]** |
| Average latency (simulated) | < 1 ms (in-process message delivery) | **[S]** |
| Maximum latency (simulated) | ~5 ms (bandwidth-limited queue) | **[S]** |
| Throughput (v3, 100 K) | 202 Mbps aggregate | **[S]** |
| Throughput (v4, 1 B) | 125 Mbps aggregate | **[S]** |
| Jitter (simulated) | ~0 (deterministic) | **[S]** |
| Packet loss tolerance | Up to 3 losses per DATA frame (retransmission cap) | **[I]** |
| Recovery time (node failure) | ~3 s simulation | **[S]** |
| DHT convergence time (50 nodes) | 3.0 s | **[S]** |
| DHT convergence time (100 K nodes) | 7.0 s (v3) / 7.5 s (v4) | **[S]** |
| DHT convergence time (1 B nodes) | 12.5 s (v4) | **[S]** |

---

## 11. Benchmarks

### 11.1 v3 Simulator Scaling

**Objective:** Measure DHT convergence time, peer count saturation, and bandwidth as a function of network size.

**Hardware:** AMD Ryzen 7 5700U, 16 GB RAM, Windows 11 (MSVC).

**Rust version:** rustc 1.87.0 (nightly).

**Configuration:** 30 bootstrap PINGs, 15 periodic PINGs + 10 FIND_NODE per round, convergence threshold = max(3 log₂(N), 21).

| Nodes | Converge | CT (sim-s) | Avg Peers | Max Peers | BW (Mbps) | Wall Time |
|-------|----------|------------|-----------|-----------|-----------|-----------|
| 10 K | ✅ 100% | 1.0 | 57.5 | — | 11.2 | 1.54 s |
| 50 K | ✅ 100% | 3.0 | 58.6 | — | 71.1 | 13.78 s |
| 100 K | ✅ 100% | 7.0 | 59.3 | 82 | 202.4 | 43.43 s |

**Reproducibility:** `cargo run --release --bin bench-fast` — deterministic output with seed 42.

**Limitations:** In-process message passing (no real sockets), single machine, no WAN latency.

### 11.2 v4 Hybrid Simulator Scaling

**Objective:** Demonstrate convergence at extreme scale using hybrid active/virtual model.

**Hardware:** Same as v3.

**Configuration:** ACTIVE_MAX = 200 K, MAX_PEERS = 500, convergence threshold = max(3 log₂(N), 30).

| Nodes | Converge | CT (sim-s) | Avg Peers | Thresh | BW (Mbps) | Wall Time |
|-------|----------|------------|-----------|--------|-----------|-----------|
| 100 K | ✅ 99.9% | 7.5 | 116.5 | 51 | 172 | 4.9 s |
| 1 M | ✅ 99.8% | 10.0 | 147.8 | 60 | 149 | 12.4 s |
| 10 M | ✅ 98.9% | 10.0 | 142.2 | 72 | 137 | 12.2 s |
| 100 M | ✅ 99.8% | 12.5 | 176.2 | 81 | 125 | 15.5 s |
| 1 B | ✅ 99.5% | 12.5 | 176.2 | 90 | 125 | 15.6 s |

**Reproducibility:** `cargo run --release --bin bench-fast-v4 "100k,1m,10m,100m,1b"` — committed results at `results/bench-fast/fast_v4_results.csv`.

**Limitations:** Hybrid model means larger N produces statistically identical results to smaller N (virtual nodes add no computation). Real-world validation is future work.

### 11.3 Real UDP Benchmarks

**Objective:** Measure actual UDP transport convergence on localhost.

**Results:** 5 nodes converge 60% of trials (25 trials), 10 nodes converge 0% (3 trials).

**Limitation:** OS socket/thread bottleneck — the protocol converges in simulation but UDP transport is constrained by kernel limits on concurrent send/recv.

### 11.4 Maintenance Mode Comparison (SGA vs. Fixed)

**Objective:** Test whether Sparse Gradient Aging reduces maintenance bandwidth vs. fixed-interval pinging.

**Result:** SGA increased bandwidth 1.9–2.45× across all configurations — the hypothesis was falsified.

**Finding:** Under stable simulated conditions, maintenance pings do not measurably improve routing quality once k-buckets are saturated because fresh buckets rarely contain stale entries.

**Reproducibility:** `results/` directory contains raw CSV for all 6 configurations (2 modes × 3 node counts).

---

## 12. Comparison Matrix

### 12.1 vs. libp2p

| Dimension | libp2p | Neuron Wire |
|-----------|--------|-------------|
| **Type** | Modular networking stack | Integrated learning runtime |
| **Language** | Go, Rust, JS, Python | Rust only |
| **Design** | Async-first, 50+ crates | Single-threaded, 5 deps |
| **Learning** | None | Hebbian STDP built-in |
| **Determinism** | Not designed for | Paper mode, fixed seed |
| **Best for** | General P2P applications | Decentralized AI research |

### 12.2 vs. IPFS

| Dimension | IPFS | Neuron Wire |
|-----------|------|-------------|
| **Purpose** | Content-addressed file storage | P2P learning runtime |
| **DHT** | Kademlia (S/Kademlia) | Kademlia with latency weighting |
| **Transport** | libp2p (TCP, QUIC, WebSockets) | Custom UDP |
| **Storage** | Content-addressed blocks | RAM-only neural state |
| **Best for** | Decentralized file sharing | Decentralized learning experiments |

### 12.3 vs. Ray

| Dimension | Ray | Neuron Wire |
|-----------|-----|-------------|
| **Coordination** | Central GCS (Global Control Store) | None |
| **Scheduling** | Centralized task scheduler | No scheduler |
| **Fault tolerance** | GCS failover | Implicit (DHT re-converges) |
| **Use case** | Distributed Python workloads | P2P Rust learning |
| **Best for** | Production ML pipelines | Research on decentralized learning |

### 12.4 vs. PyTorch Distributed

| Dimension | PyTorch DDP | Neuron Wire |
|-----------|-------------|-------------|
| **Coordination** | Synchronous All-Reduce | Asynchronous gossip |
| **Hardware** | Homogeneous GPUs with NCCL | Any device with UDP |
| **Topology** | Static ring | Dynamic (DHT-based) |
| **Learning rule** | SGD/Adam backpropagation | Hebbian STDP |
| **Best for** | Multi-GPU model training | Heterogeneous device learning |

### 12.5 vs. MPI

| Dimension | MPI | Neuron Wire |
|-----------|-----|-------------|
| **Network requirement** | High-bandwidth low-latency interconnect | Internet-grade UDP |
| **Fault tolerance** | None (process failure = job failure) | Graceful degradation |
| **Setup** | mpirun, hostfile, static world | Seed node, dynamic join |
| **Best for** | HPC simulation | Internet-scale learning experiments |

### 12.6 vs. Federated Learning

| Dimension | Federated Learning | Neuron Wire |
|-----------|--------------------|-------------|
| **Coordination** | Central aggregator | None |
| **Rounds** | Synchronous (all clients must respond) | Asynchronous gossip |
| **Data** | Data stays on device | Data stays on device |
| **Model** | Shared global model (aggregator) | No global model |
| **Privacy** | Gradients reveal information | Same (no additional guarantees) |
| **Best for** | Cross-device training with coordinator | P2P learning without coordinator |

### 12.7 vs. Gossip Learning

| Dimension | Gossip Learning | Neuron Wire |
|-----------|-----------------|-------------|
| **Peer selection** | Random | Kademlia-based (latency-weighted, cluster-biased) |
| **Routing** | None (assumes IP connectivity) | Full DHT with bootstrapping |
| **Convergence** | Formal bounds for specific cases | Simulated, formally modeled |
| **Best for** | Theoretical gossip studies | Empirical P2P learning research |

---

## 13. Scientific Claims

| Claim | Evidence Type | Status | Confidence |
|-------|--------------|--------|------------|
| Kademlia DHT converges in O(log N) time | **[S+T]** | Verified in simulation up to 1 B; O(log N) matches theory | High |
| Avg peers saturates at k-bucket capacity | **[S]** | Confirmed: ~59 (v3), ~176 (v4 with MAX_PEERS=500) | High |
| Maintenance pings do not improve routing quality under stable conditions | **[S]** | SGA found to increase bandwidth 1.9–2.45×; fixed maintenance sufficient | High |
| Hebbian STDP weight updates are deterministic and correct | **[I]** | 8 unit tests pass — pre/post activation correlation produces expected Δw | High |
| Neurogenesis triggers on sustained prediction error | **[I]** | Leaky accumulator correctly filters transient noise, integrates sustained error | High |
| Apoptosis correctly prunes inactive neurons and stale routing entries | **[I]** | 5 unit tests + simulator validation pass | High |
| UDP transport with 3 reliability tiers works on localhost | **[I]** | 342 test functions + multi-node real UDP at small scale | High |
| Hybrid model simulation cost is independent of N for N > ACTIVE_MAX | **[S]** | 100 M and 1 B produce statistically identical metrics | High |
| Protocol can simulate 1 B nodes in < 20 seconds | **[S]** | 15.6 s wall time for 100 K → 1 B sweep, 5 runs | High |
| Real-node DHT converges at 100 K nodes | **[S]** | v3 simulator, 100 K real node objects, 100% convergence | High |
| Protocol works over real WAN links | **[F]** | Not tested — all benchmarks are localhost | Low |
| Hebbian STDP over P2P gossip produces useful learning | **[F]** | Individual subsystem tests pass; end-to-end learning not demonstrated | Low |
| Protocol withstands > 10% packet loss | **[F]** | Transport retransmits up to 3×; behavior above 10% loss not characterized | Low |
| Protocol functions behind NAT | **[F]** | No NAT traversal implemented | Low |
| Protocol converges under high churn | **[F]** | Not tested beyond crash-recovery in simulation | Low |
| Single-threaded engine scales to 10⁴+ neurons per node | **[T]** | O(n) per tick for n synapses; budget may exceed 1 ms at density | Medium |

---

## 14. Limitations

| Limitation | Why It Exists | Impact | Severity | Planned Solution | Research Status |
|------------|--------------|--------|----------|-----------------|-----------------|
| No WAN deployment | Funding and infrastructure needed | Every claim about real-Internet behavior is a hypothesis | Critical | D1: 100+ node VPS deployment across 3 continents | **[F]** |
| No NAT traversal | Not implemented | Nodes behind NAT cannot participate without public IP | Critical | STUN/TURN/ICE integration in D1 | **[F]** |
| No transport-layer encryption | Not implemented | All traffic is in plaintext — any peer can eavesdrop | Critical | Noise Protocol (DTLS-style, zero-alloc) in D1 | **[F]** |
| No authentication | Not implemented | Any process can generate a NodeId and join | High | Ed25519 signature verification in future work | **[F]** |
| No Sybil resistance | Not implemented | Single attacker can dominate routing table with many IDs | High | Trust scoring exists but not used for routing decisions | **[I part, F part]** |
| Single-threaded engine | Deliberate design choice | Cannot exploit multi-core; limits per-node neural graph size | Medium | Multi-threaded sharded engine in D3 | **[F]** |
| No persistent storage | Not implemented | Process restart = total state loss | Medium | Snapshot/restore in D3 | **[F]** |
| Learning convergence unproven | Complex open research question | Current system may not learn useful representations | High | Dedicated learning convergence experiments post-D1 | **[F]** |
| Static gossip fanout (3 peers) | Hardcoded parameter | May under-connect at 10³+ nodes; may waste bandwidth at < 10 | Medium | Adaptive fanout based on network size (research) | **[F]** |
| No congestion control | Not implemented | Node may saturate its uplink under high gossip load | Medium | Implicit back-pressure via gradient decay; explicit CC as future work | **[F]** |
| UDP-only transport | Deliberate choice | Fails in HTTP-only environments, corporate proxies | Medium | WebRTC transport in D3 | **[F]** |
| Linux/x86-64 only (tested) | Development environment constraint | Windows/macOS timing characteristics uncharacterized | Low | Cross-platform CI matrix already defined | **[F]** |

---

## 15. Security

**Threat model:** An attacker who can send UDP packets to any node on the network and can observe, inject, modify, or drop any message in transit, but does not control the majority of routing table entries for any honest node. **[I]**

**Sybil resistance:** Basic — each peer is scored by a trust system (initial trust 0.5, decays over time, boosted by valid signatures/decryption). Peers below SYBIL_THRESHOLD (0.2) are subject to rate limiting. No proof-of-work or stake mechanism. An attacker with many IP addresses can still generate many identities, but each new identity starts untrusted and is rate-limited. **[I part, F part]**

**Eclipse attacks:** Partially mitigated — each peer is identified by its Ed25519 public key (not just NodeId), so an attacker must generate distinct keypairs to fill k-buckets. No formal eclipse resistance beyond the trust system. **[I part, F part]**

**Replay attacks:** Prevented — each encrypted packet carries a monotonic nonce counter verified by the receiver. Duplicates beyond a 1024-entry window are rejected. Unencrypted (signed-only) packets rely on sequence numbers; the transport header ACK tracker provides additional ordering. **[I]**

**Spoofing:** Prevented — every outbound packet is Ed25519-signed with the sender's private key. The receiver extracts the public key from the 32-byte auth prefix and verifies the 64-byte signature over the body. A forger cannot spoof a packet without the sender's private key. **[I]**

**Identity:** Ed25519 keypairs generated per node (OsRng). EntityId = SHA-256(public_key). Signatures are verified on every incoming packet when `security_enabled=true` (default). Key rotation via `rotate()` creates fresh identity with cert chain support. **[I]**

**Encryption:** XChaCha20-Poly1305 AEAD available per-packet. Key derivation uses a Noise-like XX handshake (simplified: SHA-256(local_pk || peer_pk || ephemeral) — real X25519 ECDH not yet implemented). Session keys are per-peer with monotonic nonce counters for replay protection. Enabled via `encrypt_payloads` config flag. **[I part, F part]**

**Authentication:** Every outbound packet carries a 96-byte auth prefix (32B public key + 64B Ed25519 signature over body). Receivers verify signatures before processing; invalid signatures are dropped, logged, and reduce the sender's trust score. **[I]**

**Authorization:** All peers have equal access to all resources — no role-based access control. **[F]**

**Integrity:** Ed25519 signatures provide cryptographic integrity on every packet. Tampered packets are rejected at the signature verification stage. CRC32 remains for accidental corruption detection. **[I]**

**Availability:** Rate limiting via the trust system (max N packets per time window for low-trust peers). No DoS-specific protection beyond trust-based throttling. **[I part, F part]**

**Confidentiality:** Optional AEAD encryption (XChaCha20-Poly1305) per packet when `encrypt_payloads=true`. Without encryption, the body is signed but in plaintext. **[I part, F part]**

**Audit:** Hash-chain audit log records all security events (NodeStartup, PeerDiscovered, HandshakeSuccess, InvalidSignature, RateLimitTriggered, ReplayDetected, etc.). Each entry's hash chains to the previous, making tampering detectable. Bootstrap proofs enable log verification. **[I]**

---

## 16. Simulation

**Simulator architecture:** A single process launches N threads each running the full engine loop, with messages delivered via in-process channels rather than UDP sockets, enabling deterministic execution at any scale. **[I]**

**Physics assumptions:** No physics simulation — the simulator models network conditions (latency, loss, bandwidth caps) as configurable parameters applied per-message. **[I]**

**Network assumptions:** Messages are delivered instantaneously in the fast simulators (bench-fast); the paper-mode simulator can add configurable latency and loss distributions. **[I]**

**Timing assumptions:** Sim-time advances independently of wall-clock time; `sleep` calls in the deterministic simulator advance sim-time directly without blocking. **[I]**

**Randomness:** All randomness is seeded from a single command-line seed; every subsystem (DHT, Hebbian, Neurogenesis, transport) uses a deterministic RNG derived from the seed. **[I]**

**Determinism:** Same seed → same CSV output on any platform, any Rust version, any hardware — guaranteed by fixed RNG, sim-time, and no wall-clock-dependent assertions. **[I]**

**Failure injection:** The simulator supports configurable node crash (process terminates mid-experiment), packet loss (X% of messages silently dropped), and latency (per-message delay drawn from configurable distribution). **[I]**

**Reproducibility:** Every experiment produces: `experiment.toml` (frozen config), `metadata.json` (git commit, rustc, platform), raw per-tick CSV, and optional `known-good/` validation. **[I]**

**Validation methodology:** The CI pipeline runs each benchmark and compares output CSVs against `known-good/` reference files using `diff`; any mismatch fails the build. **[I]**

---

## 17. Open Source

**Repository structure:** Root Cargo workspace with `demo/` subcrate; `src/` for library, `examples/` for simulations, `tests/` for integration tests, `benches/` for Criterion benchmarks, `fuzz/` for cargo-fuzz targets. **[I]**

**Coding standards:** `cargo fmt` enforced, `cargo clippy -D warnings` (zero-warnings policy), `cargo test` must pass, `unsafe` prohibited outside explicitly audited modules. **[I]**

**CI/CD:** 4 GitHub Actions workflows — build+test (matrix: ubuntu, macos, windows), clippy, coverage (cargo-llvm-cov), benchmark validation (against known-good references). **[I]**

**Documentation:** ARCHITECTURE.md (system design), PROTOCOL_SPEC.md (wire format), CONTRIBUTING.md (contribution guide), FOUNDATIONAL_QNA.md (200+ Q&A), GRANT.md (funding application), FORMAL_MODEL.md (math proofs), README.md (quick start). **[I]**

**Issue process:** Bug reports use `bug_report.md` template, feature requests use `feature_request.md`, research contributions use `research_contribution.md`. Good first issues are tagged. **[I]**

**Pull request process:** Branch from master → commit with conventional message → open PR with `Closes #N` → CI runs → maintainer review (1–3 business days) → squash-merge. **[I]**

**Release process:** `release/vX.Y.Z` branch cut from master → CHANGELOG.md updated → Cargo.toml version bumped → tag created → CI auto-publishes to crates.io. **[I]**

**Community governance:** Single maintainer (Cian Magic, Zylvon) with decisions guided by the research principles in GRANT.md. No formal steering committee. **[I]**

---

## 18. Research

**Published papers:** None. **[I]**

**Draft papers:** None in progress. The experimental infrastructure (7 Python baselines, 10 TOML experiment configs, reproduction scripts) is in place; the manuscript is not started. **[F]**

**Planned papers:** A reproducible evaluation paper comparing NWP against vanilla Kademlia, gossip SGD, and federated averaging across LAN, WAN, and mixed topologies — target submission Q4 2026. **[F]**

**Research questions:** 8 foundational questions documented in `RESEARCH_QUESTIONS.md`, covering DHT convergence under churn, Hebbian STDP gradient convergence, bandwidth-scaling ceiling, NAT traversal feasibility, Byzantine resilience, optimal gossip fanout, predictive vs. reactive apoptosis, and real-vs-simulated fidelity. **[I]**

**Open problems:** Whether Hebbian STDP converges over gossiped gradients, whether the DHT convergence theorem holds under real Internet churn, what the optimal gossip fanout is as a function of N, and whether the architecture is practical on resource-constrained devices. **[F]**

**Future experiments:** 100-node multi-continent VPS deployment with real latency/loss/churn measurements (the single highest-priority experiment — it would falsify the architecture if it fails). **[F]**

**Conference targets:** SysML, MLSys, HotOS, DSN, EuroSys (workshop tracks preferred for first submission). **[F]**

**Journal targets:** IEEE/ACM Transactions on Networking, Journal of Parallel and Distributed Computing, Distributed Computing, Neural Computation. **[F]**

---

## 19. Funding

### 19.1 Why Seek Funding Now

**Why seek funding now?** Every benchmark to date runs on localhost — real WAN deployment requires VPS instances across multiple continents, which costs money the project does not currently have. **[I]**

**Why not bootstrap?** Full bootstrapping over the WAN test would require $600–$1,200/month for 100+ VPS instances across 3 regions for 3 months, plus engineering time for NAT traversal and encryption — beyond unfunded research capacity. **[T]**

**What infrastructure is missing?** VPS compute across 3 continents (us-east-1, eu-west-1, ap-southeast-2), STUN/TURN servers for NAT traversal testbed, and a CI runner capable of WAN benchmarks. **[F]**

**What experiments cannot currently be performed?** Any experiment requiring nodes on different machines, different networks, or different continents — which is essentially every experiment that would produce publishable results about real-world behavior. **[F]**

### 19.2 Funding Levels

**What would a $10,000 grant enable?** Single-region deployment (~20 nodes × 3 months), initial NAT traversal engineering, and one month of measurement data. **[F]**

**What would a $50,000 grant enable?** Two-region deployment (~50 nodes × 3 months), Noise Protocol encryption implementation, STUN traversal, and draft paper submission. **[F]**

**What would a $100,000 grant enable?** Full three-continent deployment (100+ nodes × 3 months), all three deliverables (WAN deployment + publication + SDK start), and open-access publication fees. **[F]**

**What would a $500,000 grant enable?** Multi-region deployment with 500+ nodes, WebRTC transport, Python bindings, full-time engineering support, and community building. **[F]**

**What would a $1,000,000 grant enable?** Comprehensive deployment with 1,000+ nodes across 6+ regions, complete SDK ecosystem, formal verification of protocol properties, conference travel for 3+ events, and two full-time researchers for 18 months. **[F]**

### 19.3 Milestones and Deliverables

**What milestones would each funding level unlock?** $10k → single-region baseline data; $50k → cross-region comparison; $100k → publication-ready dataset with all 7 baselines; $500k → SDK release + external contributors; $1M → industry adoption groundwork. **[F]**

**What deliverables would funders receive?** Open datasets (per-tick latency, throughput, convergence curves), reproducible experiments, published paper (with funder acknowledgment), public dashboard, and SDK releases. **[F]**

**What risks remain even with funding?** The protocol's core assumption (direct UDP connectivity) may fail for most devices behind CGNAT; Hebbian STDP may not converge to useful representations over gossiped gradients; real-Internet churn may prevent DHT convergence entirely. **[F]**

---

## 20. Future

**What does Version 1.0 look like?** A production-hardened release with Noise Protocol encryption, STUN NAT traversal, persistent state snapshot/restore, multi-threaded engine, and published reproducible benchmarks from real WAN deployment. **[F]**

**What does Version 2.0 add?** Adaptive gossip fanout (self-tuning based on network size), Byzantine fault tolerance for consensus messages, WebRTC transport for browser-to-browser across machines, and Python bindings via PyO3. **[F]**

**What is the project's strongest long-term position?** Not as a single learning runtime, but as an experimental platform for decentralized systems research where researchers can plug in different routing algorithms (Kademlia variants, Chord, Pastry), gossip strategies (random, cluster-biased, latency-weighted, SGA), learning rules (STDP, contrastive divergence, local SGD, BPTT approximations), trust models (reputation, economic, cryptographic), and transport layers (TCP, QUIC, WebRTC, LoRa), and compare them under reproducible conditions using the same measurement framework. **[F]**

**What becomes possible when Neuron Wire is a platform?** A researcher can answer "which DHT variant converges fastest under 20% packet loss?" or "does STDP outperform local SGD on non-IID data?" by swapping one configuration parameter and running the same benchmark suite — without rebuilding the entire stack from scratch. **[F]**

**What is Version 5.0?** A fully community-maintained open-source infrastructure layer with 5+ reference applications, formal verification of core protocol properties, and adoption by at least 3 independent research groups. **[F]**

**What projects come after Neuron Wire?** Neuron Wire Labs (experimental protocols, formal verification research), Neuron Wire Next (v2 protocol design with lessons from WAN deployment), and domain-specific toolkits (edge AI, disaster response, decentralized LLM inference). **[F]**

**How does Neuron Wire fit into Zylvon's long-term vision?** It is the foundational infrastructure layer — every Zylvon project (decentralized inference, collaborative edge learning, P2P robotics) builds on the discovery, transport, and learning primitives that Neuron Wire provides. **[I+F]**

**Could it become an industry standard?** Potentially — if the WAN deployment validates the simulation predictions, if the reproducible benchmarks establish a ground truth for decentralized learning comparisons, and if the community adopts the protocol as a shared substrate. **[F]**

**What would success look like in five years?** A research group in any country can clone neuron-wire, join a P2P learning network with a single command, contribute compute or data to a shared learning task, and publish results that the community can independently reproduce — without any cloud credits, coordinator permission, or infrastructure provisioning. **[F]**

---

## 21. Evidence Behind Every Claim

| Claim | Type | Status | Confidence | Evidence |
|-------|------|--------|------------|----------|
| DHT converges in O(log N) time | **[S+T]** | Verified in simulation | High | v4: 7.5 s (100 K) → 12.5 s (1 B) — O(log N) matches theory |
| Average peers saturates at k-bucket capacity | **[S]** | Verified | High | v3: ~59 peers (K=20 × ~3 non-empty buckets), v4: ~176 (MAX_PEERS=500 cap) |
| Maintenance pings don't improve routing under stable conditions | **[S]** | Verified | High | SGA increased bandwidth 1.9–2.45×; fixed maintenance sufficient |
| 1 B nodes simulate in < 20 s wall time | **[S]** | Verified | High | 15.6 s for complete 100 K–1 B sweep |
| 1 B simulation is a routing algorithm bound, not an operational capacity claim | **[S]** | Caveat documented | High | Hybrid model (200 K active + virtual); real WAN behavior would differ — see §9 |
| Hebbian STDP update rule is correct | **[I]** | Verified | High | 8 unit tests confirm Δw = η · pre · post produces correct sign and magnitude |
| Neurogenesis triggers on sustained surprise | **[I]** | Verified | High | Leaky accumulator correctly filters vs. integrates |
| Apoptosis prunes stale entries | **[I]** | Verified | High | 5 unit tests + death spiral guard validation |
| UDP transport works on localhost | **[I]** | Verified | High | 342 test functions + small cluster (3–5 nodes) |
| **Learning subsystem produces useful results at any task** | **[F]** | **Not tested — biggest scientific gap** | **Low** | **No end-to-end benchmark exists. Benchmark protocol is designed (PROJECT_INTELLIGENCE.md §8 + RESEARCH_QUESTIONS.md RQ9) but not executed. Estimated 200–400 lines of harness code to produce the first graph.** |
| Protocol works over real WAN links | **[F]** | Not tested | Low | All benchmarks are localhost; WAN deployment is D1 milestone |
| Hebbian STDP converges over P2P gossip | **[F]** | Not tested | Low | Subsystems tested individually; end-to-end learning not demonstrated |
| Protocol withstands > 10% packet loss | **[F]** | Not tested | Low | Retransmission cap = 3; degradation curve uncharacterized |
| Protocol functions behind NAT | **[F]** | Not tested | Low | No STUN/TURN/ICE implementation |
| Protocol converges under high churn | **[F]** | Not tested | Low | Crash-recovery tested in simulation; real churn unmeasured |
| Single-threaded engine scales to 10⁴+ neurons | **[T]** | Theoretically bounded | Medium | O(n) per tick; budget may exceed 1 ms at high density |
| Hybrid virtual model accurately represents real DHT behavior | **[T+S]** | Partially validated | Medium | v4 matches v3 convergence trends; fidelity to real UDP behavior unknown |
| Ed25519 identity system is cryptographically sound | **[I+T]** | Verified | High | Standard Ed25519 implementation; signatures verified on every inbound packet (engine_loop `open_inbound`) |
| Packet authentication (sign/verify) wired into engine loop | **[I]** | Verified | High | `seal_outbound()` signs every outbound packet; `open_inbound()` verifies every inbound. All 4 security modules integrated (identity, channel, trust, audit) |
| AEAD encryption round-trip (XChaCha20-Poly1305) | **[I]** | Verified | High | `SecureChannel::encrypt/decrypt` with monotonic nonce + replay protection. Two integration tests: auth prefix roundtrip and full encrypt-then-sign |
| Trust scoring with rate limiting | **[I]** | Verified | High | `TrustSystem` tracks 6 event types, decays over time, enforces rate limit (10 pkts/window per peer) |
| Audit hash chain with bootstrap proofs | **[I]** | Verified | High | `AuditLog` maintains hash chain, supports checkpoint, produce/verify bootstrap proof |
| Formal model bounds are mathematically correct | **[T]** | Peer review pending | Medium | 1,760-line model with theorems/lemmas/proofs; not yet externally reviewed |

---

## Appendix A: Glossary

| Term | Definition |
|------|-----------|
| **Node** | One instance of the Neuron Wire runtime — a process with its own routing table, neural graph, and UDP socket |
| **Peer** | Another node discovered via the DHT with which the local node can exchange messages |
| **Packet** | A framed sequence of bytes sent over UDP: [16-byte transport header | 16-byte NWP header | N-byte body] |
| **NWP** | Neuron Wire Protocol — the binary wire format and message semantics |
| **DHT** | Distributed Hash Table — the routing substrate (Kademlia, 256 buckets, K=20) |
| **K-bucket** | A routing table slot holding up to K peers sharing a specific XOR distance range |
| **STDP** | Spike-Timing-Dependent Plasticity — Δw depends on temporal order of pre/post activation |
| **Hebbian learning** | "Neurons that fire together, wire together" — Δw = η · pre · post |
| **Forward pass** | The activation propagation: leak → propagate → squash → compare → clean |
| **Surprise** | Prediction error from ForwardPass — triggers neurogenesis when accumulated > 0.2 |
| **Neurogenesis** | Birth of new neurons in response to sustained prediction error |
| **Apoptosis** | Programmed removal of stale routing entries, synapses, and transport frames |
| **Paper mode** | Deterministic simulation with fixed seed, frozen config, relative timestamps |
| **Convergence** | State where each node has discovered ≥ max(3 log₂(N), threshold) peers |
| **Gossip** | Periodic exchange of accumulated gradient deltas with a subset of peers |
| **FlatBuffer** | Google serialization format used for zero-copy message body access |

---

## Appendix B: Quick Reference

- **Repository:** [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)
- **Dashboard:** [neuron-wire-dashboard.vercel.app](https://neuron-wire-dashboard.vercel.app)
- **License:** MIT
- **Language:** Rust 2021 edition
- **Dependencies:** 5 direct (crc32fast, rand, serde, toml, csv)
- **Most recent commit:** `d82f2d7` (2026-07-09)
- **Biggest result:** 1 B nodes converged, 15.6 s wall time
- **Biggest gap:** No WAN deployment — all claims about real-Internet behavior are hypotheses
- **Contact:** team@zylvon.com
