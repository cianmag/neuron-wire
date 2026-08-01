# Neuron Wire — Risks & Limitations

**Document type:** Grant package annex · **Purpose:** Pre-answer reviewer objections
**Project:** Neuron Wire Protocol (NWP) · **Repo:** [github.com/cianmag/neuron-wire](https://github.com/cianmag/neuron-wire) · **Org:** Zylvon
**Date:** 2026-07-31

> *"A limitation documented is stronger than an assumption hidden."*
> This document states, in plain terms, what Neuron Wire has **not** yet proven. Every item below is a
> known, named, bounded gap — not an unknown unknown. Each has a current mitigation, a reason it does
> not invalidate the project, and a concrete way funding closes it. A reviewer who reads this document
> knows exactly what they are funding and what success looks like.

---

## Risk 1 — No geographically distributed deployment yet

**Description.** Every convergence result in the repository was produced by simulation on a single
machine, or by small real-UDP tests on localhost (real UDP benchmarks cap at ~10–25 nodes because of
OS thread/socket limits; 5-node trials converged 60% of the time). No NWP node has ever run on the
public Internet across real links, NATs, and ISPs. This is the single largest gap between the current
state and the project's claims.

**Current mitigation.** (1) The v3 simulator runs real `Node` objects with real in-process message
passing, not statistical abstractions — 100 K nodes converge 100% in 7.0 simulated seconds, with the
caveat "simulated ≠ real" documented in `STATS.md` and `PROJECT_INTELLIGENCE.md`. (2) The protocol has
production-shaped deployment artifacts: Docker images (Alpine + scratch, non-root, HEALTHCHECK),
Ansible provisioning (systemd, firewall, monitoring), 3-tier bootstrap (config → DNS seeds →
hardcoded → passive), and STUN-based external address discovery. (3) The experimental protocol —
hypotheses, variables, success and failure criteria — is committed to `GRANT.md` *before* deployment,
so the WAN experiment will be timestamped and immutable.

**Why it does not invalidate the project.** A WAN deployment is a validation step, not a design
unknown. The algorithmic properties claimed (O(log N) convergence, ~O(1) bandwidth per node) are
properties of the routing and trust algorithms, which are exercised honestly in simulation and
analyzed in the formal model. Every serious P2P system — BitTorrent, IPFS, Kademlia itself — was
first validated in simulation before WAN trials. What is unproven is *operational* behavior under
real network conditions, and that is precisely the question funding would answer.

**What funding unlocks.** A 20-node, three-region pilot (us-east-1, eu-west-1,
ap-southeast-2), a sustained 7-day mesh, a public metrics dashboard, and a published WAN dataset —
positive and negative results alike.

---

## Risk 2 — Simulator results may differ from WAN results

**Description.** Deterministic in-process simulation abstracts away OS networking, kernel scheduling,
NAT behavior, packet loss, congestion, and heterogeneous hardware. Claims like "1 B nodes converge in
12.5 s" (v4 hybrid) are algorithm-level results and must not be read as operational capacity.

**Current mitigation.** (1) The claim discipline is explicit: the repo states "this evaluates the
routing *algorithm's* theoretical bound, not operational capacity." (2) Two independent evidence
tracks exist — the simulator (scale) and localhost real-UDP benchmarks (reality check) — and the
discrepancy between them is reported, not hidden (real UDP maxes at ~10–25 nodes on localhost due to
OS limits; the fast sim proves the protocol converges; OS transport is the constraint). (3) Language
throughout the docs is hedged precisely: "under the evaluated conditions," "in the tested
configuration." (4) Convergence thresholds include conservative headroom (`max(3·log₂ N, 21)`,
`MAX_PEERS = 500`), so the design is not tuned to the edge of simulation.

**Why it does not invalidate the project.** The claims made are scoped to what was evaluated:
routing-algorithm convergence and bandwidth scaling. Simulation is the standard first stage of
protocol research, and the difference between simulation and reality is a *known, measurable*
quantity — that is why the funding plan includes a WAN experiment with pre-committed success
criteria. If WAN results differ, that is a publishable finding, not a failure of the project.

**What funding unlocks.** Real-WAN validation experiments, public datasets, and the paper that
resolves the simulation-vs-reality question either way.

---

## Risk 3 — NAT traversal remains incomplete

**Description.** NWP implements STUN-based external address discovery (`stun.rs`, tested), but there
is no TURN relay, no hole-punching validation against real consumer NATs (full cone, restricted,
symmetric), and no measured traversal success rate. A meaningful fraction of consumer devices sit
behind symmetric NATs that defeat standard UDP hole punching.

**Current mitigation.** (1) STUN is implemented and covered by tests. (2) The 3-tier bootstrap gives
nodes multiple independent paths to first contact. (3) The architecture does not depend on universal
traversal: any node with a reachable address can join, and server-reachable (VPS) nodes can form a
mesh even if consumer nodes cannot accept inbound connections. (4) The gap is stated openly in the
audit and roadmap rather than papered over.

**Why it does not invalidate the project.** The core research claim — collaborative learning without
centralized coordination — requires that *some* set of devices can reach each other, not that every
device behind every NAT can. NAT traversal is a reachability-extension feature on top of that core.
Its incompleteness is a scope decision with a documented relay path forward, not a design flaw.

**What funding unlocks.** A multi-NAT testbed across real ISPs, a TURN relay deployment, traversal
success-rate measurements, and (if needed) fallback relay design for symmetric-NAT devices.

---

## Risk 4 — Learning quality needs broader datasets

**Description.** The distributed learning layer (Hebbian STDP, gradient gossip, neurogenesis/
apoptosis) has been evaluated on internal synthetic tasks and its own convergence behavior — not on
standard machine-learning benchmarks at scale, and not head-to-head against federated learning on
real datasets.

**Current mitigation.** (1) Seven Python baselines exist in `baselines/` (federated averaging,
decentralized SGD, parameter server, Horovod All-Reduce, BitTensor, Ray distributed, unified
comparison framework). (2) Learning parameters (learning rate, decay, gradient half-life,
neurogenesis threshold) are documented and reproducible via 10 TOML experiment configs. (3) The
project's claims about learning are stated as infrastructure demonstrations, not SOTA results.

**Why it does not invalidate the project.** NWP's research contribution is the *coordination
infrastructure* — discovery, transport, trust, and reproducible experimentation for decentralized
learning — not a new model architecture. Learning quality is a utility metric for that
infrastructure, and the honest position is that this metric is not yet measured on standard
benchmarks. That is a measurement gap, and the measurement plan exists.

**What funding unlocks.** A standard benchmark suite (MNIST/CIFAR-scale tasks distributed over the
mesh), baseline comparisons at scale, and public release of the datasets and configs for
reproducibility.

---

## Risk 5 — Security has not received external audit

**Description.** The security layer is strong by design and internally audited — Ed25519
authentication on every packet, XChaCha20-Poly1305 AEAD with ephemeral X25519 forward secrecy,
trust-weighted rate limiting, trust persistence across restarts, hash-chained tamper-evident audit
log — but no third party has reviewed it. An internal audit (`docs/AUDIT_REPORT.md`) and an
itemized security checklist (`docs/SECURITY_CHECKLIST.md`, 47/61 items ✅, 10 ⚠️, 4 ❌) are
published, which is more than most prototypes do — and still not an external audit.

**Current mitigation.** (1) Published internal audit with prioritized findings (P0–P3) and a fix
track. (2) Four fuzz targets on parsers with `catch_unwind`; zero-warning `clippy -D warnings`
policy; all 13 `unsafe` blocks annotated with `SAFETY:` comments; minimal dependency surface
(31 direct deps, zero external runtime deps in the core node). (3) The four ❌ checklist items are
named: key zeroization, traffic-analysis padding, read-only container filesystem, audit-log size
caps. (4) The threat model is documented (per-packet auth, replay detection, per-IP connection
caps, Sybil thresholds).

**Why it does not invalidate the project.** Absence of an external audit is a *process and funding*
gap, not a design defect. The codebase is structured to be auditable — small modules, documented
invariants, a 485-line wire spec, a 1,760-line formal model — and the internal audit gives an
external auditor a map. Claiming security maturity would be dishonest; claiming an auditable,
documented, internally-reviewed design is accurate.

**What funding unlocks.** A third-party security audit earmarked from grant funds, completion of the
four ❌ items, fuzzing in CI, and OSS-Fuzz integration.

---

## Risk 6 — Formal proofs depend on model assumptions

**Description.** `FORMAL_MODEL.md` (1,760 lines, 17 sections) proves DHT convergence, trust
convergence, redundancy bounds, partition tolerance, and entropy bounds — under stated model
assumptions (e.g., bounded churn, message-delivery characteristics, honest-majority trust dynamics).
If reality violates an assumption, the corresponding guarantee weakens.

**Current mitigation.** (1) Assumptions are stated explicitly in the model — they are visible and
falsifiable, not hidden. (2) The simulator acts as an empirical check on the model: the 100 K-node
convergence result matches the O(log N) prediction. (3) The proofs are written for a specific,
named protocol configuration, so they are checkable against the implementation.

**Why it does not invalidate the project.** Every formal result in distributed systems rests on
model assumptions — that is the nature of the field. The scientific value is in making assumptions
explicit and testing their consequences, which this project does. A proof under explicit assumptions
plus simulation evidence is the strongest evidence a research prototype can offer at this stage.

**What funding unlocks.** Extending proofs to weaker assumptions, adversary-model refinement, and a
formal-verification pass (e.g., TLA+/model-checking the state machine against the formal model).

---

## Risk 7 — Sybil resistance is mitigation, not elimination

**Description.** NWP resists Sybil attacks through reputation: trust scores bounded to [0,1] with
time-based decay, persistence across restarts (attackers don't get fresh starts), trust-weighted rate
limiting, Sybil-threshold detection (score < 0.2 = untrusted), and per-IP connection caps. None of
this *eliminates* Sybils — an attacker with enough identities, time, and patience can still gain
influence. There is no cryptographic identity scarcity (no proof-of-work or stake).

**Current mitigation.** (1) Trust persistence denies fresh-start attacks. (2) Per-IP limits
(`per_ip_max_peers`, default 10) bound single-source flooding. (3) Trust convergence is proven in
the formal model under stated assumptions. (4) The threat model is documented; the checklist tracks
eclipse-attack mitigation as "⚠️ Kademlia k-bucket routing helps, no formal proof."

**Why it does not invalidate the project.** In permissionless networks, Sybil resistance is always
economic or statistical — proof-of-work, proof-of-stake, and reputation are all *mitigations*, and
none eliminates Sybils. NWP's reputation approach is the standard non-cryptographic tool, and the
project is honest that it raises the cost of attack rather than making it impossible. A permissionless
network that claims Sybil *elimination* is the one that should not be trusted.

**What funding unlocks.** Sybil attack simulations at scale, adversarial modeling, comparison against
stake-based alternatives, and incentive-design research to quantify and raise the cost of sustained
attack.

---

## Risk Matrix

Likelihood and impact are assessed **as of 2026-07-31, pre-funding**.

| # | Risk | Likelihood | Impact | Mitigation strength (current) | Residual risk | Directly closed by funding? |
|---|------|------------|--------|-------------------------------|---------------|------------------------------|
| 1 | No geo-distributed deployment | **High** (certain — current state) | **High** | Medium — sim + deploy artifacts ready | Medium: protocol may need WAN tuning | **Yes — D1 testbed** |
| 2 | Simulator ≠ WAN results | **High** | Medium | Medium — dual evidence tracks, hedged claims | Medium: magnitude of delta unknown | **Yes — WAN experiments** |
| 3 | NAT traversal incomplete | **High** | Medium | Low–Medium — STUN only, no relay | Medium–High for consumer devices | **Yes — relay + testbed** |
| 4 | Learning quality unproven on broad datasets | Medium | Medium | Medium — 7 baselines, reproducible configs | Medium | **Yes — benchmark suite** |
| 5 | No external security audit | **High** | Medium | **High** — published internal audit, fuzzing, minimal deps | Low–Medium | **Yes — audit earmark** |
| 6 | Formal proofs rest on assumptions | Medium | Low | **High** — assumptions explicit, sim validates model | Low | Partial — weaker-assumption proofs |
| 7 | Sybil resistance is mitigation, not elimination | Medium | Medium | Medium — trust + persistence + per-IP caps | Medium | Partial — attack simulations |

### Reading the matrix

- **No row is a design-fatal unknown.** The highest-impact risk (1) is also the most predictable:
  the project has never run on the WAN, and the funding plan's first deliverable exists to change
  exactly that.
- **Every row is a validation or verification gap, not a contradiction of the core claim.** The core
  claim — that collaborative learning can be coordinated without a central operator, with
  documented security and trust properties — is supported by simulation, formal analysis, and
  internal audit, all of which are published and reproducible.
- **Funding maps 1:1 to risks.** Rows 1–5 are directly closed by the requested work; rows 6–7 are
  partially closed by research extension. No requested funding goes to unlisted work.

---

## Why this document makes the application stronger

1. **It pre-answers the standard reviewer questions.** Every objection a technical reviewer can
   raise is named, bounded, and paired with a mitigation and a funding outcome.
2. **It demonstrates research discipline.** The project's philosophy — evidence over features,
   limitations documented over assumptions hidden — is applied to the grant application itself.
3. **It defines success honestly.** The WAN experiment has pre-committed success *and failure*
   criteria; negative results will be published. A funder is not buying a guaranteed outcome; they
   are buying a rigorous answer to a well-posed question, produced by infrastructure that already
   exists and already works as far as it claims to.
