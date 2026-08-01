# Neuron Wire: Foundational Q&A

> Every answer is one sentence. Updated 2026-07-09 · Commit `5a739f9` · Repository: [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire)

---

## 1. The Elevator Pitch

**What is Neuron Wire in one sentence?** An open-source Rust framework that enables any device to discover peers, exchange learning signals, and participate in collaborative AI without centralized infrastructure.

**Explain Neuron Wire in 20 words.** Open-source P2P framework for decentralized AI — DHT peer discovery, custom UDP transport, Hebbian learning, reproducible simulation.

**Explain it to a 10-year-old.** It lets computers teach each other by sharing small hints instead of sending all their data to one big server.

**Explain it to a software engineer.** A single-threaded non-blocking engine loop integrating Kademlia DHT routing, custom reliable UDP transport, and STDP-based neural computation — 400 KHz–1 MHz tick rate with zero external runtime dependencies.

**Explain it to an AI researcher.** A reproducible research platform for studying whether Hebbian STDP over Kademlia-routed P2P topologies can produce collaborative learning signals under realistic network conditions.

**Explain it to a VC.** Infrastructure for decentralized AI — any device with a network connection can participate in collaborative machine learning without servers, centralized coordination, or a single point of control.

**Explain it to a grant reviewer.** A deterministically simulated and formally modeled P2P learning runtime, demonstrated to converge at up to 1 B nodes in simulation, seeking funding for first real Internet deployment and reproducible publication.

**Explain it to someone who has never heard of distributed systems.** A program that lets many computers work together on a problem by sharing pieces of information directly with each other, with no single computer in charge.

**What problem exists today that Neuron Wire solves?** Distributed AI requires centralized infrastructure — parameter servers, GPU clusters, or federated aggregators — creating single points of failure, access barriers, and privacy risks that a fully P2P approach avoids.

**Why should anyone care?** If collaborative learning can work without centralized infrastructure, any device anywhere can participate in AI development, and the question of whether this is practical is unanswered by published evidence.

---

## 2. The Core Problem

**What is fundamentally broken in today's AI infrastructure?** Every collaborative learning system either requires a central coordinator, a static topology, or homogeneous hardware, making AI inaccessible from regions without reliable cloud infrastructure.

**Why are centralized GPU clusters a limitation?** They concentrate control, pricing, and access in a single entity, excluding participants who lack the capital or connectivity to provision cloud instances.

**Why are parameter servers a bottleneck?** Every gradient passes through a central node that can fail, be censored, or become saturated, and the server's operator controls who participates.

**Why are current distributed ML systems difficult to scale?** All-Reduce assumes a known static participant set, federated learning requires a central aggregator, and gossip protocols lack formal convergence guarantees under real Internet conditions.

**What inspired Neuron Wire?** The observation that decentralized P2P routing (BitTorrent, Ethereum) and biological learning rules (STDP) have never been integrated into a single runtime with reproducible benchmarking.

**What existing technologies influenced the design?** Kademlia DHT (2002) for routing, FlatBuffers (2014) for zero-copy serialization, Hebbian STDP (1949/1996) for learning, and biological neuroplasticity for graph adaptation.

**What assumptions does Neuron Wire challenge?** That distributed learning requires a coordinator, that reliability requires TCP, that routing and learning can be designed independently, and that simulation results cannot predict real-Internet behavior.

**Why hasn't this problem already been solved?** Networking, distributed systems, and machine learning are studied independently, and no existing open-source project combines all three in a single auditable runtime with reproducible benchmarks.

**What happens if Neuron Wire never exists?** Decentralized AI research continues without a standardized, reproducible, open-source substrate for P2P learning experiments — each research group builds its own from scratch.

**What changes if it succeeds?** Any device with a network connection can join a collaborative learning network without provisioning infrastructure, trusting a coordinator, or exposing private data — and publish reproducible evidence that the approach works or doesn't.

---

## 3. One-Sentence Definitions

**What is a node?** A single instance of the Neuron Wire runtime — a running process with its own routing table, neural graph, and UDP socket.

**What is a peer?** Another node discovered via the DHT with which the local node can exchange messages.

**What is a packet?** A framed sequence of bytes sent over UDP carrying a transport header (16 bytes), NWP message header (16 bytes), and optional FlatBuffer body.

**What is the Neuron Wire protocol?** A binary wire protocol (NWP) defining how neural state — activations, gradients, spike events, consensus votes — is serialized, routed, decayed, and merged across a dynamic P2P network.

**What is gradient exchange?** The process of serializing accumulated synaptic weight deltas into a FlatBuffer frame and dispatching them to up to 3 DHT peers every N ticks.

**What is distributed learning?** Each node learns locally via Hebbian STDP and shares a subset of its weight updates with peers, such that the collective improves without any node holding the full model.

**What is gossip learning?** A peer samples one or more known nodes, sends accumulated gradients, and receives updates in return — no global aggregation step required.

**What is the DHT?** A distributed hash table implementing latency-weighted Kademlia with 256 k-buckets (K=20 each), XOR-distance routing, and DNS seed bootstrap.

**What is Hebbian learning?** A local weight update rule where the connection between two neurons strengthens when they activate simultaneously: Δw = η · pre · post.

**What is neurogenesis?** The birth of new neurons triggered when accumulated prediction error exceeds 0.2, adding a fresh 256-bit EntityId with random initial synapse weights.

**What is apoptosis?** Programmed removal of routing entries, synapses, and frames that have not been used within configurable timeouts or exceeded failure thresholds.

**What is prediction error?** The mean squared error between a neuron's actual and predicted activation, emitted by the ForwardPassSystem each tick as a scalar surprise signal.

**What is paper mode?** A deterministic simulation mode with fixed seed, frozen parameters, relative timestamps, and output validated against known-good reference CSVs.

**What is convergence?** The state where each node has discovered at least max(3 log₂(N), threshold) peers, indicating the routing graph is connected and stable.

**What is a routing bucket?** One of 256 k-buckets storing up to 20 nodes whose XOR distance from the local node falls within a specific power-of-two range.

---

## 4. Technical Summary

**What language is it written in?** Rust 2021 edition, compiled with `opt-level="z"`, LTO, and stripped to produce a single statically linked binary.

**Why Rust?** Memory safety without garbage collection, zero-cost abstractions, guaranteed no undefined behavior outside one zero-copy crate, and a type system that eliminates entire classes of networking bugs.

**Why not C++?** C++ lacks Rust's memory safety guarantees without a GC, has no standard build system, and its type system does not prevent use-after-free or data races at compile time.

**Why not Go?** Go's garbage collector introduces latency spikes unacceptable for a 1 MHz real-time tick loop, and its goroutine model obscures the single-threaded execution semantics the engine depends on.

**Why not Python?** Python cannot sustain a 1 MHz tick rate, requires a GIL for thread safety, and its runtime overhead makes deterministic nanosecond-scale simulation impractical.

**Why no async runtime?** A single-threaded recv loop with 1 ms read timeout eliminates scheduler jitter, avoids 50+ transitive crate dependencies, and delivers maximum throughput on 512 MB RAM shared CPU instances.

**Why single-threaded?** The engine loop owns the UDP socket, routing table, and neural graph on one thread — no locks, no channels in the hot path, and deterministic tick ordering.

**Why UDP?** UDP avoids TCP's head-of-line blocking where one lost packet stalls the entire stream, and lets the protocol choose which messages require reliability (DATA, CONSENSUS) and which do not (SPIKE, COMMAND).

**Why not TCP?** TCP's in-order delivery guarantees are unnecessary for gradient exchange where stale data is exponentially decayed, and TCP's congestion control adds latency variance that confounds timing-dependent learning experiments.

**Why not QUIC?** QUIC is a new protocol, not yet universally available, implemented in Rust via indirect system calls, and its stream abstraction is unnecessary when the protocol already handles its own reliability tiers.

**Why custom reliability?** Only 2 of 7 message types need retransmission; TCP or QUIC would retransmit everything, wasting bandwidth on SPIKE and READINESS frames where fresh data supersedes old.

**Why no external dependencies?** A research prototype with 5 direct dependencies minimizes supply-chain risk, simplifies auditing, and ensures the binary can be built from a fresh checkout in under 2 minutes without network access.

**Why deterministic simulation?** Deterministic execution (fixed seed, no wall-clock-dependent assertions) means the same command produces the same CSV output on every platform — the foundation of reproducible research.

**Why Kademlia?** Kademlia provides O(log N) routing with proven performance (BitTorrent, Ethereum), XOR-distance bucket placement guarantees global reachability, and the algorithm fits in ~400 lines of implementation.

**Why STDP?** STDP is the best-understood biological learning rule with precise mathematical formulation (Δw depends on spike timing) and requires no backpropagation through time, making it suitable for decentralized, asynchronous execution.

---

## 5. Architecture

**What are the major subsystems?** DHT routing, UDP transport, ForwardPass neural computation, Hebbian STDP learning, Neurogenesis (neuron birth), Apoptosis (neuron death), identity management, trust scoring, audit logging, and deterministic simulation.

**How many modules exist?** 41 source files (~20,900 lines of Rust) exposing the core subsystems: DHT routing, UDP transport, ForwardPass neural computation, Hebbian STDP learning, neurogenesis, apoptosis, identity, trust, audit, simulation.

**Which module starts first?** The UDP socket opens and the engine loop begins draining inbound packets before any DHT or neural subsystem is initialized.

**Which module is most performance-critical?** The engine loop (`engine_loop.rs`) — every tick must complete within 1 μs–1 ms to sustain the 400 KHz–1 MHz tick rate.

**Which module consumes the most CPU?** The ForwardPass system, which iterates over every neuron and synapse each tick to propagate activations and compute prediction error.

**Which module consumes the most memory?** The DHT routing table, which can hold up to 256 buckets × 20 entries = 5,120 peer records, each with IP address, latency EMA, fail count, and metadata.

**Which module handles networking?** The transport layer (`transport.rs` and `udp_transport.rs`) owns the UDP socket, manages the ACK sliding window, and implements retransmission for reliable message types.

**Which module handles learning?** The Hebbian STDP module (`hebbian.rs`) and the ForwardPass module (`forward_pass.rs`) together implement neural computation and weight adaptation.

**Which module handles routing?** The DHT module (`dht.rs`) implements latency-weighted Kademlia routing with 256 buckets, periodic maintenance, and DNS seed bootstrap.

**Which module handles identity?** The identity module (`identity.rs`) manages Ed25519 key pairs for signing messages, and the trust module (`trust.rs`) implements reputation-based Sybil resistance.

---

## 6. Performance

**Maximum simulated nodes?** 1 B nodes (v4 hybrid simulator, 15.6 s wall time, 99.5% convergence).

**Maximum real nodes?** ~10–25 on localhost UDP (OS socket/thread limited); WAN deployment is the next milestone.

**Largest experiment completed?** 1 B nodes in v4 hybrid simulation with 200 K active nodes, 176 average peers, 125 Mbps bandwidth.

**Tick rate?** ~400 KHz–1 MHz on commodity x86-64 hardware when idle; ~1 KHz deterministic simulation rate in paper mode.

**Average latency?** Sub-millisecond for in-process delivery in the fast simulators; not yet measured across real networks.

**Peak bandwidth?** 202 Mbps aggregate at 100 K nodes in v3 simulator (2.0 Mbps/node); 125 Mbps at 1 B nodes in v4 hybrid (~0.6 Mbps per active node).

**CPU usage?** 0% when idle (OS blocks on `recv_from()` timeout); single core fully utilized during active simulation.

**Memory usage?** ~500 KB per active node for routing table (5,120 peer records × ~100 bytes) plus neural graph proportional to neuron count.

**Binary size?** ~2–4 MB after `opt-level="z"`, LTO, and strip — a single statically linked executable.

**Startup time?** < 100 ms from `cargo run --release` to first tick on modern x86-64 hardware.

**Shutdown time?** < 10 ms — the engine loop receives a stop signal and exits after completing the current tick.

**Packet throughput?** ~22,000 packets/second/node at 50 nodes in SGA mode (v2 simulator benchmark).

**Messages per second?** ~1,300–22,000 depending on network size and maintenance mode.

**Peer discovery speed?** Full routing table convergence in 3.0 s for ≤ 50 nodes, 7.0 s for 100 K nodes (v3), 12.5 s for 1 B nodes (v4).

**Time to convergence?** O(log N) measured: 3.0 s at 50 nodes, 7.5 s at 100 K, 10.0 s at 1 M, 12.5 s at 1 B.

---

## 7. Benchmarks

**Which benchmarks exist?** DHT convergence time, bandwidth consumption, packet counts, maintenance mode comparison (fixed vs. SGA), fault injection, scaling curves (100 to 1 B nodes), and per-subsystem unit tests.

**Which benchmarks are reproducible?** All nine — every benchmark uses a fixed seed, produces deterministic CSV output, and is validated against known-good reference values in CI.

**What hardware were they run on?** Single machine, AMD Ryzen 7 5700U with 16 GB RAM, Windows 11 with MSVC toolchain.

**Which benchmark is your strongest?** The v4 hybrid scaling benchmark demonstrating 99.5% convergence at 1 B nodes in 15.6 s with constant per-node bandwidth.

**Which benchmark is weakest?** The real UDP benchmark, which maxes out at ~10–25 nodes due to OS socket/thread limits rather than protocol limitations.

**Largest successful simulation?** 1 B nodes in v4 hybrid mode with 200 K active nodes, converging to 176 average peers.

**Average convergence rate?** 100% for v3 up to 100 K nodes; ≥ 98.9% across all v4 scales from 100 K to 1 B (threshold = max(3 log₂(N), 30) peers).

**Network overhead?** ~40 bytes per message (16 transport + 16 NWP header + 8 CRC/framing), with body size depending on message type.

**Routing efficiency?** ~59 average peers per node at 100 K nodes (saturating at K-bucket capacity) with only 15 PINGs + 10 FIND_NODE per round.

**Packet delivery rate?** 100% in simulation (in-process message passing); real UDP delivery rate depends on network conditions and is untested.

**Failure recovery time?** ~3 s from node failure to routing table convergence in simulation; untested on real networks.

**Node join time?** ~3 s for a new node to discover ≥ threshold peers in simulation.

**Node removal time?** ~600 s (stale timeout) for passive eviction; 3 missed pings for active failure detection.

**Scalability curve?** O(log N) convergence time and O(N log N) total messages (empirically validated 100 to 1 B).

**Complexity?** DHT routing O(log N) lookups, routing table space O(K × B) = O(20 × 256) constant per node, bootstrap communication O(N log N) empirical.

---

## 8. Comparison

**Compared to libp2p?** Libp2p is a modular networking stack with 50+ crates and async-first design; Neuron Wire is a single-threaded runtime with integrated learning — not a replacement but a specialization.

**Compared to IPFS?** IPFS is a distributed file system using content-addressed storage; Neuron Wire is a P2P learning runtime — they share a DHT concept but solve different problems.

**Compared to BitTorrent?** BitTorrent uses Kademlia DHT only for trackerless peer discovery; Neuron Wire integrates DHT routing directly into a learning engine loop where routing decisions affect gradient flow.

**Compared to MPI?** MPI is a message-passing standard for tightly coupled HPC clusters with reliable interconnects; Neuron Wire targets heterogeneous devices over unreliable Internet links.

**Compared to Ray?** Ray provides distributed task scheduling with a centralized coordinator (GCS); Neuron Wire provides decentralized learning with no coordinator.

**Compared to TensorFlow Distributed?** TF Distributed uses gRPC for gradient exchange between workers coordinated by a chief; Neuron Wire uses custom UDP with no chief.

**Compared to PyTorch DDP?** PyTorch DDP implements synchronous All-Reduce over NCCL requiring homogeneous GPUs with low-latency interconnects; Neuron Wire runs on any device with a UDP port.

**Compared to Horovod?** Horovod wraps MPI All-Reduce with a TensorFlow-compatible API; Neuron Wire has no external dependencies and no MPI requirement.

**Compared to Parameter Servers?** A parameter server holds the authoritative model state and coordinates updates from workers — a single point of failure and bandwidth bottleneck.

**Compared to Federated Learning?** Federated learning requires a central aggregator that averages model weights from client devices; Neuron Wire has no aggregator and no central round synchronization.

**Compared to Swarm Learning?** Swarm Learning replaces the aggregator with blockchain-based consensus but still requires a network-wide agreement step each round.

**Compared to Gossip Learning?** Gossip learning randomly exchanges gradients between peer pairs — Neuron Wire adds Kademlia routing structure to gossip selection, biasing toward cluster-similar peers.

**Compared to Kubernetes?** Kubernetes is a container orchestration platform for centralized deployments; Neuron Wire is a P2P protocol requiring no orchestration.

**Compared to Kubernetes + Ray?** Combined they provide centralized job scheduling on managed infrastructure; Neuron Wire provides decentralized peer-to-peer learning on unmanaged devices.

**Compared to Spark?** Spark performs in-memory data processing across a cluster with a driver node coordinating all tasks; Neuron Wire has no driver and no master node.

---

## 9. Capabilities

**What can Neuron Wire do today?** Discover peers via Kademlia DHT, exchange messages over custom UDP with three reliability tiers, run Hebbian STDP learning locally, simulate up to 1 B nodes deterministically, and export reproducible CSVs of every metric.

**What can't it do?** Operate over real Internet links (untested), encrypt traffic, traverse NATs, persist state across restarts, or run on multi-core architectures.

**What is experimental?** The entire neural computation pipeline (ForwardPass + Hebbian + neurogenesis + apoptosis) — unit tests verify each subsystem independently, but end-to-end collaborative learning has not been demonstrated.

**What is production-ready?** Nothing — the project is a research prototype with documented security gaps and no WAN validation.

**What is simulation-only?** All performance claims about convergence time, bandwidth scaling, and fault tolerance — these have only been measured in the deterministic simulator on localhost.

**What requires future work?** NAT traversal, wire encryption, persistent storage, multi-threaded engine, WebRTC transport, Python bindings, and any WAN deployment.

**Can it survive node failures?** In simulation, yes — the DHT re-converges within ~3 s after nodes are removed.

**Can it handle packet loss?** The reliable queue retransmits up to 3 times (DATA) or 5 times (CONSENSUS), but degradation under >10% loss is untested.

**Can it recover from partitions?** In simulation, nodes re-discover each other via periodic maintenance pings when the partition heals.

**Can nodes join dynamically?** Yes — a new node bootstraps via seed addresses, runs iterative FIND_NODE, and populates its routing table within seconds (simulation).

**Can nodes leave dynamically?** Yes — leaving nodes are detected via stale timeout (600 s) or failed ping threshold (3 misses) and removed from routing tables.

**Can malicious nodes attack it?** Yes — the current v0.3.1 prototype adds Ed25519 packet authentication, trust-based rate limiting, and optional AEAD encryption, but lacks formal Sybil resistance, BFT consensus, and DoS amplification protection.

**Can it run on Raspberry Pi?** In theory — the single-threaded engine targets 512 MB RAM and compiles to ARM — but this has not been tested.

**Can it run on phones?** In theory — the Rust code compiles to ARM64 and the UDP module works on Android/Linux — but no mobile build exists.

**Can it run on browsers?** Yes — a 101 KB WASM demo exists demonstrating P2P neural network between two browser tabs via BroadcastChannel.

---

## 10. Limitations

**Biggest limitation?** Every benchmark runs on localhost — no real Internet deployment has been attempted, so every claim about WAN behavior is a hypothesis.

**Biggest engineering challenge?** Building a deterministic simulation framework that captures enough real-network complexity (NAT, jitter, loss, asymmetric routing) to predict WAN behavior.

**Biggest research challenge?** Determining whether Hebbian STDP over gossiped gradients converges to useful representations under realistic network conditions — currently unvalidated.

**Biggest security concern?** Zero transport-layer encryption — any peer on the network can read all messages in plaintext.

**Biggest networking challenge?** NAT traversal — without STUN/TURN, every node must be directly reachable on UDP, which excludes most home and mobile devices.

**Biggest scaling challenge?** The single-threaded engine cannot exploit multi-core systems, placing an upper bound on per-node neural graph size regardless of available cores.

**Biggest memory bottleneck?** The full neural graph (neurons, synapses, activations) lives entirely in RAM with no snapshot/restore — process restart means total state loss.

**Biggest CPU bottleneck?** ForwardPass iterates over every neuron and synapse each tick — a dense neural graph of 10⁴+ neurons will not fit within a 1 ms tick window on a single core.

**Biggest latency bottleneck?** The UDP read timeout (1 ms) sets the minimum tick interval and caps the tick rate regardless of how fast computation completes.

**Biggest assumption?** That nodes can communicate directly via UDP — this fails behind symmetric NATs, corporate firewalls, and CGNAT deployments.

**What breaks first?** The transport layer under high packet loss — retransmission caps at 3 for DATA frames, and gradient decay means late arrivals contribute almost nothing.

**What remains unsolved?** Whether any of the simulation results generalize to the real Internet — this is the defining open research question of the project.

---

## 11. Security

**Does it encrypt traffic?** Not by default — `encrypt_payloads=false` in the standard config. When enabled, each packet is encrypted with XChaCha20-Poly1305 AEAD (session key via simplified Noise-like handshake).

**How are identities verified?** Every packet carries a 32-byte Ed25519 public key + 64-byte signature in its auth prefix; the receiver verifies the signature against the public key before processing the body.

**How are nodes authenticated?** Every node is identified by its Ed25519 public key; the identity subsystem (identity.rs) generates keypairs and signs every outbound packet. There is no certificate authority or PKI — identities are self-signed.

**Can Sybil attacks happen?** Partially mitigated — each peer is scored by a trust system (initial 0.5, boosted by valid signatures, decayed for failures). Peers below SYBIL_THRESHOLD (0.2) are rate-limited to 10 packets/s. However, an attacker with many IP addresses can still generate many Ed25519 keypairs at low cost.

**Can Eclipse attacks happen?** Yes — an attacker controlling enough NodeIds can surround a target node and isolate it from honest peers. Each node is identified by its Ed25519 public key, mitigating keyless NodeId spoofing.

**Can replay attacks happen?** Encrypted sessions use monotonic nonce counters verified against a ring buffer; signed-only packets use transport sequence numbers without strict verification.

**Can packets be forged?** No — every packet carries a 96-byte auth prefix (32B public key + 64B Ed25519 signature) verified on receipt. A forger cannot produce a valid signature without the sender's private key.

**What trust model is used?** Reputation scoring (trust.rs) with 6 event types, configurable decay, and per-peer rate limiting at 10 packets/window for untrusted peers.

**How is integrity verified?** Every packet carries an Ed25519 signature over its body, providing cryptographic integrity. A CRC32 checksum on each NWP frame provides an additional accidental-corruption check.

**How is replay prevented?** Encrypted packets use monotonic nonce counters verified against a 1024-entry ring buffer; replayed nonces are rejected. Signed-only (unencrypted) packets fall back to transport header sequence numbers without strict replay verification.

---

## 12. Simulation

**What simulator exists?** A deterministic paper-mode simulator (`simulator.rs`, 1,515 lines) that launches N nodes, runs them for T ticks, and exports all metrics to CSV.

**What does paper mode simulate?** A configurable network of N nodes running the full engine loop (DHT + transport + neural computation) in a single process with in-process message delivery.

**What is deterministic?** Everything — RNG is seeded from command-line seed, tick timing uses sim-time rather than wall clock, and all outputs are bit-for-bit identical across runs with the same seed.

**Which variables are configurable?** Node count, duration, gossip interval, tick interval, seed, maintenance mode (fixed/SGA), topology, failure injection parameters, and learning hyperparameters.

**Which metrics are collected?** Convergence time, peer count (average, max, min), bandwidth in/out, packet counts, apoptosis events, gradient delivery rate, and prediction error over time.

**Which events are logged?** Node join, node leave, peer discovery, ping/pong, FIND_NODE response, gradient exchange, neurogenesis, apoptosis, retransmission, and failure detection.

**Can failures be injected?** Yes — the simulator supports configurable node crash, packet loss, and latency injection during a run.

**Can latency be simulated?** Yes — per-message latency can be configured with configurable distribution (fixed or uniform range).

**Can bandwidth be limited?** Yes — the simulator enforces per-node bandwidth caps and queues messages that exceed the limit.

**Can topology change?** Yes — nodes can be added or removed mid-experiment, and partitions can be injected and healed programmatically.

---

## 13. Research

**Which hypotheses are being tested?** (1) Sparse Gradient Aging reduces DHT maintenance bandwidth — falsified, SGA increased bandwidth 1.9–2.45×; (2) Kademlia DHT maintains O(log N) convergence under churn; (3) Hebbian STDP converges over gossiped gradients in dynamic P2P topologies.

**Which papers exist?** None published yet — the project is in the pre-publication phase with all experimental infrastructure in place.

**Which papers are planned?** A reproducible evaluation paper comparing NWP against vanilla Kademlia, gossip SGD, and federated averaging across LAN, WAN, and mixed topologies.

**Which conferences fit this work?** SysML, MLSys, HotOS, DSN, EuroSys, and P2P networking workshops.

**Which journals fit this work?** IEEE/ACM Transactions on Networking, Journal of Parallel and Distributed Computing, Distributed Computing, and Neural Computation.

**Which open problems remain?** Whether simulation results generalize to real networks, whether Hebbian STDP converges with stale gradients, what scaling ceiling exists for single-threaded routing, and whether the architecture is practical on resource-constrained devices.

**What experiments are next?** A 100-node multi-continent VPS deployment measuring real latency, loss, churn, and convergence — the experiment that would prove the architecture wrong if it fails.

**Which claims are proven?** DHT convergence time O(log N) up to 1 B nodes in simulation, maintenance pings do not improve routing quality under stable simulated conditions, and the hybrid model makes simulation cost independent of N for N > active_max.

**Which claims remain hypotheses?** That the protocol works over real Internet links, that Hebbian STDP produces useful learning over gossiped gradients, and that the system withstands real churn rates.

**Which claims require real-world validation?** All of them — every performance, convergence, and fault tolerance claim is currently simulation-only.

---

## 14. Roadmap

**Current stage?** Research prototype (v0.3.0) with complete simulation infrastructure, formal model, and all baseline comparisons implemented — no WAN deployment.

**Next milestone?** Deliverable 1: Deploy 100+ nodes across AWS free-tier in 3 continents (us-east-1, eu-west-1, ap-southeast-2) and measure real-Internet behavior.

**Six-month goal?** Published reproducible evaluation paper with open datasets and one-command figure reproduction (Deliverable 2).

**One-year goal?** Developer SDK with crates.io publishing, Python bindings, and 5 reference applications (Deliverable 3).

**Five-year vision?** A community-maintained open-source infrastructure layer for decentralized AI used by research groups and edge deployments worldwide.

**First real deployment?** 100+ VPS nodes across 3 continents with public live dashboard — contingent on grant funding.

**First VPS deployment?** AWS free-tier t3.micro or equivalent bursting instances — cost is the only blocker.

**First university collaboration?** Not yet established — the project is seeking academic partners for the WAN deployment and publication phase.

**First published paper?** Target: Q4 2026 workshop submission (SysML, MLSys, HotOS, or DSN).

**First external contributors?** Zero external contributions as of 2026-07-09 — the project has been a single-contributor effort to date.

---

## 15. Funding

**Why do you need funding?** To deploy 100+ VPS nodes across 3 continents for the first real-Internet measurement — currently the project has only been tested on localhost.

**What happens without funding?** The project continues as unfunded research at smaller scale (10–25 nodes on localhost, simulator studies, and incremental engineering).

**What happens with $10k?** A 100-node single-region deployment, one month of measurement data, and initial NAT traversal engineering.

**What happens with $100k?** Full three-continent deployment (100+ nodes × 3 months), Noise Protocol encryption, STUN traversal, publication fees, and reproducible datasets.

**What happens with $1M?** Multi-region deployment with 1,000+ nodes, WebRTC transport, Python bindings, full-time engineering team, community building, and conference travel.

**How many nodes could each funding level deploy?** $10k → ~20 nodes × 3 months, $100k → 100+ nodes × 3 months, $1M → 1,000+ nodes × 12 months.

**Which experiments become possible?** At $100k+: latency distribution benchmarks across continents, churn experiments with real NAT-rebind behavior, packet loss characterization, and comparison of all 7 baselines on real hardware.

**What scientific questions would funding answer?** Does the DHT converge under real Internet conditions? Can cross-continent gradient delivery sustain < 50% loss? Do the simulation predictions match reality? Is decentralized collaborative learning practical outside the lab?

---

## 16. Open Source

**Why is it open source?** Reproducible research requires open code, open data, and open methodology — closed-source claims about decentralized learning are inherently untrustworthy.

**Why should someone contribute?** To help answer a real open research question — can P2P collaborative learning work over real Internet conditions? — and to gain experience with Rust systems programming, P2P protocols, and reproducible benchmarking.

**How can someone contribute?** Via CONTRIBUTING.md — prioritize evidence-strengthening contributions (benchmarks, experiments, formal analysis) over feature additions.

**What's the first good issue?** Running the existing benchmarks on a different machine and validating reproducibility — listed under `good first issue` on GitHub.

**How is quality maintained?** Four CI workflows enforce `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and benchmark validation against known-good reference values.

**How are releases managed?** Releases are cut from `release/vX.Y.Z` branches with CHANGELOG updates, Cargo.toml version bumps, and automatic crates.io publishing — managed by the maintainer.

**How are benchmarks verified?** Every experiment is deterministic — the CI pipeline runs each benchmark and compares output CSVs against `known-good/` reference files.

**How is reproducibility ensured?** Deterministic simulation mode (paper mode), fixed seeds, frozen TOML configs, metadata capture (git commit, rustc version, platform), and CI validation against known-good outputs.

---

## 17. Vision

**If Neuron Wire succeeds, what changes in AI?** Collaborative AI becomes infrastructure anyone can access — no cloud credits, no coordinator permission, no data leaving your device.

**If Zylvon succeeds, what does the company become?** The organization that built the open infrastructure layer making decentralized AI practical — the Linux Foundation of P2P intelligence.

**What is the ultimate vision for decentralized intelligence?** Any Internet-connected device can participate in a planetary-scale neural network that learns collaboratively without central control, single points of failure, or trust requirements.

**What is the biggest technical risk?** The architecture's core assumption — that direct UDP connectivity is available between arbitrary nodes — fails for most devices behind CGNAT, corporate firewalls, or mobile networks.

**What is the biggest scientific breakthrough you're aiming for?** Demonstrating that Hebbian STDP over P2P gradient gossip converges at useful learning rates under real Internet conditions — a phenomenon that would suggest the network itself can be a computational substrate.

**What legacy do you want Neuron Wire to leave?** A reproducible, open-source benchmark for decentralized learning that every future project — including competitors — uses as ground truth for their own claims.

**Why should the world invest time, talent, or money in this project?** Because the question of whether decentralized collaborative learning works over real networks is unanswered by published evidence, and answering it — regardless of outcome — advances the field more than building another closed system.
