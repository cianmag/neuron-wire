# ML Research — Tier 3 Machine Learning Infrastructure

> **Tier 3 — Machine Learning**
> Advanced ML techniques integrated into the decentralized neural computation substrate.
> Every feature has: mathematical specification, implementation reference, benchmark integration, and baseline comparison.

---

## Table of Contents

1. [Adaptive Learning Rates](#1-adaptive-learning-rates)
2. [Gradient Compression](#2-gradient-compression)
3. [Weight Pruning & Sparse Tensors](#3-weight-pruning--sparse-tensors)
4. [Dynamic Activation Functions](#4-dynamic-activation-functions)
5. [Attention-Like Routing](#5-attention-like-routing)
6. [Online Continual Learning](#6-online-continual-learning)
7. [Catastrophic Forgetting Benchmarks](#7-catastrophic-forgetting-benchmarks)
8. [Knowledge Distillation](#8-knowledge-distillation)
9. [Distributed Replay Buffers](#9-distributed-replay-buffers)
10. [Local Memory Modules](#10-local-memory-modules)
11. [Prediction Uncertainty & Bayesian Confidence](#11-prediction-uncertainty--bayesian-confidence)
12. [Curiosity Scheduling & Intrinsic Motivation](#12-curiosity-scheduling--intrinsic-motivation)
13. [Meta-Learning](#13-meta-learning)
14. [Federated Baseline](#14-federated-baseline)
15. [Decentralized SGD Baseline](#15-decentralized-sgd-baseline)
16. [Parameter Server Baseline](#16-parameter-server-baseline)
17. [Ray Baseline](#17-ray-baseline)
18. [Horovod Baseline](#18-horovod-baseline)
19. [Bittensor Baseline](#19-bittensor-baseline)
20. [Benchmark Framework](#20-benchmark-framework)

---

## 1. Adaptive Learning Rates

### 1.1 Motivation

The current Hebbian STDP uses a fixed learning rate $\eta = 0.01$ for all synapses. This is suboptimal: frequently-updated synapses benefit from smaller rates to prevent oscillation, while rarely-updated synapses benefit from larger rates to accelerate convergence.

### 1.2 AdaHebbian — Per-Synapse AdaGrad

Each synapse $w_{ij}$ maintains an accumulated squared gradient $G_{ij}$:

$$G_{ij}^{(t+1)} = G_{ij}^{(t)} + (\Delta w_{ij}^{(t)})^2$$

where $\Delta w_{ij}^{(t)} = \eta \cdot a_i^{(t)} \cdot a_j^{(t)}$ is the Hebbian update (before decay). The adaptive learning rate is:

$$\eta_{ij}^{(t)} = \frac{\eta_0}{\sqrt{G_{ij}^{(t)} + \epsilon}}$$

where $\eta_0$ is the base learning rate and $\epsilon = 10^{-8}$ prevents division by zero.

**Update rule:**

$$w_{ij}^{(t+1)} = w_{ij}^{(t)} + \frac{\eta_0}{\sqrt{G_{ij}^{(t)} + \epsilon}} \cdot a_i^{(t)} \cdot a_j^{(t)} - \lambda \cdot w_{ij}^{(t)}$$

### 1.3 RMSHebbian — RMSProp-Style

Replace the unbounded sum $G_{ij}$ with an exponentially weighted moving average:

$$\mathbb{E}[g^2]_{ij}^{(t+1)} = \rho \cdot \mathbb{E}[g^2]_{ij}^{(t)} + (1 - \rho) \cdot (\Delta w_{ij}^{(t)})^2$$

where $\rho = 0.9$ is the decay rate. The adaptive learning rate becomes:

$$\eta_{ij}^{(t)} = \frac{\eta_0}{\sqrt{\mathbb{E}[g^2]_{ij}^{(t)} + \epsilon}}$$

### 1.4 AdamHebbian — Adam-Style

Combines RMSProp with momentum. Maintains first and second moment estimates:

$$\begin{aligned}
m_{ij}^{(t+1)} &= \beta_1 m_{ij}^{(t)} + (1 - \beta_1) \Delta w_{ij}^{(t)} \\
v_{ij}^{(t+1)} &= \beta_2 v_{ij}^{(t)} + (1 - \beta_2) (\Delta w_{ij}^{(t)})^2 \\
\hat{m}_{ij} &= m_{ij} / (1 - \beta_1^t) \\
\hat{v}_{ij} &= v_{ij} / (1 - \beta_2^t) \\
\eta_{ij}^{(t)} &= \frac{\eta_0}{\sqrt{\hat{v}_{ij}} + \epsilon} \\
w_{ij}^{(t+1)} &= w_{ij}^{(t)} + \eta_{ij} \cdot \hat{m}_{ij} - \lambda \cdot w_{ij}^{(t)}
\end{aligned}$$

Default: $\beta_1 = 0.9$, $\beta_2 = 0.999$, $\epsilon = 10^{-7}$.

### 1.5 Integration

```rust
pub enum AdaptiveLR {
    Fixed(f32),        // original: η = const
    AdaGrad { eps: f32 },
    RMSProp { rho: f32, eps: f32 },
    Adam { beta1: f32, beta2: f32, eps: f32 },
}

// Per-synapse optimizer state
pub struct OptimizerState {
    // AdaGrad accumulator or RMSProp EWMA
    g2: Option<f32>,
    // Adam moment estimates
    m: Option<f32>,
    v: Option<f32>,
    t: u64,  // Adam bias correction counter
}
```

### 1.6 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(dm^2)$ per tick | $O(dm^2)$ per tick |
| **Memory** | $O(2dm^2)$ (states) | $O(2dm^2)$ |
| **Communication** | unchanged | unchanged |

---

## 2. Gradient Compression

### 2.1 Motivation

Gossip bandwidth is limited to $K_{\text{syn}}$ synapses per frame per gossip target. For large networks ($m > 10^3$), sending all gradients each round is infeasible. Compression reduces communication while preserving learning quality.

### 2.2 Top-K Sparsification

Instead of sending all $dm^2$ gradients, each node sends only the $k$ largest-magnitude gradients:

$$\mathcal{T}_K = \{(i,j,w_{ij}) : |w_{ij}| \text{ is in the top } K \text{ of } |\nabla|\}$$

**Error feedback.** Accumulate compression error locally:

$$e_{ij}^{(t+1)} = e_{ij}^{(t)} + \Delta w_{ij}^{(t)} - \text{compress}(\Delta w_{ij}^{(t)})$$

The next tick applies both the new gradient and the accumulated error:

$$\tilde{\Delta} w_{ij}^{(t+1)} = \Delta w_{ij}^{(t+1)} + e_{ij}^{(t+1)}$$

**Compression ratio:** $r = \frac{dm^2}{K}$. With K = 20 (gossip frame capacity) and $dm^2 = 10^4$:

$$r = \frac{10^4}{20} = 500\times$$

### 2.3 Quantization

Each transmitted weight is quantized from f32 (4 bytes) to:

| Scheme | Bits per weight | Size per weight | Compression vs f32 |
|--------|----------------|-----------------|-------------------|
| Float32 | 32 bits | 4 B | 1× |
| Float16 | 16 bits | 2 B | 2× |
| BFloat16 | 16 bits | 2 B | 2× |
| Int8 | 8 bits | 1 B | 4× |
| Binary | 1 bit | 0.125 B | 32× |
| Stochastic binary | 1 bit (Bernoulli) | 0.125 B | 32× |

**Stochastic quantization function:**

$$Q(w) = \text{sign}(w) \cdot \begin{cases}
\left\lceil|w|\right\rceil & \text{with prob } |w| - \lfloor|w|\rfloor \\
\left\lfloor|w|\right\rfloor & \text{otherwise}
\end{cases}$$

This is unbiased: $\mathbb{E}[Q(w)] = w$.

### 2.4 Combined Compression Pipeline

```
Raw gradients → Top-K selection → Quantization → Send
                                                      ↓
Receive → Dequantize → Error accumulation → Apply
```

### 2.5 Integration

```rust
pub struct GradientCompression {
    pub method: CompressionMethod,
    pub top_k: usize,
    pub quantize_bits: u8,
    pub error_feedback: bool,
}

pub enum CompressionMethod {
    None,
    TopK { k: usize },
    Quantize { bits: u8 },
    TopKThenQuantize { k: usize, bits: u8 },
}
```

---

## 3. Weight Pruning & Sparse Tensors

### 3.1 Unified Pruning Framework

Extend micro-pruning (§1.4 of FORMAL_MODEL.md) with multiple strategies:

| Strategy | Criteria | Use case |
|----------|----------|----------|
| Magnitude | $|w_{ij}| < \theta$ | Default (existing) |
| Gradient-based | $|\Delta w_{ij}| \cdot T < \theta$ | Rarely-updated weights |
| Activation-based | $|a_i^{(t)} \cdot a_j^{(t)}| < \theta$ | Dead neurons |
| SNIP (single-shot) | $|w_{ij} \cdot \partial \mathcal{L} / \partial w_{ij}|$ | Initialization pruning |
| Lottery ticket | Iterative magnitude pruning + reset | Finding subnetworks |

### 3.2 Sparse Tensor Storage

Replace the current dense weight matrix with a structured sparse format:

```rust
pub struct SparseTensor<T> {
    /// Non-zero values
    pub values: Vec<T>,
    /// Row indices (COO format)
    pub row_indices: Vec<u32>,
    /// Column indices
    pub col_indices: Vec<u32>,
    /// Shape (rows, cols)
    pub shape: (usize, usize),
}

impl SparseTensor<f32> {
    pub fn matmul(&self, vec: &[f32]) -> Vec<f32> { /* O(nnz) */ }
    pub fn outer_update(&mut self, pre: &[f32], post: &[f32]) { /* O(nnz) */ }
}
```

**Compressed Sparse Row (CSR) format for weight matrix:**

```rust
pub struct CSRMatrix {
    /// Non-zero weight values
    pub values: Vec<f32>,
    /// Column index for each value
    pub col_indices: Vec<u32>,
    /// Start index of each row in values/col_indices (length m+1)
    pub row_ptr: Vec<usize>,
    /// Number of rows, columns
    pub shape: (usize, usize),
}
```

### 3.3 Sparse Operations

| Operation | Dense | Sparse (CSR) | Speedup at $d=0.1$ |
|-----------|-------|-------------|---------------------|
| Matmul $\mathbf{W}\mathbf{v}$ | $O(m^2)$ | $O(dm^2)$ | 10× |
| Outer product update | $O(m^2)$ | $O(dm^2)$ | 10× |
| Weighted sum (sparse) | $O(m^2)$ | $O(dm^2)$ | 10× |
| Transpose matmul $\mathbf{W}^\top\mathbf{v}$ | $O(m^2)$ | $O(dm^2)$ | 10× |

### 3.4 Integration with Existing Code

Replace `HashMap<(EntityId, EntityId), f32>` in `SynapseMap` with `CSRMatrix`. The CSR format natively supports:
- Row-major forward pass ($\mathbf{W}\mathbf{a}$)
- Efficient column access for backprop ($\mathbf{W}^\top \delta$)
- Cache-friendly sequential memory access
- SIMD-accelerable dot products via `dot_product` on sparse rows

---

## 4. Dynamic Activation Functions

### 4.1 Motivation

The current tanh activation is fixed. Different layers and learning stages benefit from different activation shapes: early layers may need sharper nonlinearities, while output layers may benefit from smoother functions.

### 4.2 Trainable Activation Functions

| Function | Equation | Parameters | Shape |
|----------|----------|-----------|-------|
| PReLU | $\max(\alpha x, x)$ | $\alpha \in \mathbb{R}$ (per-neuron) | Leaky ReLU with learnable slope |
| Swish | $x \cdot \sigma(\beta x)$ | $\beta \in \mathbb{R}$ (per-neuron) | Smooth, non-monotonic |
| GELU | $x \cdot \Phi(x)$ | — | Smooth ReLU approximation |
| Tanh (existing) | $\tanh(x)$ | — | Default, bounded [-1, 1] |
| Adaptive Tanh | $\tanh(\gamma x)$ | $\gamma \in \mathbb{R}^+$ | Learnable slope |
| Softplus | $\ln(1 + e^x)$ | — | Smooth ReLU, unbounded |
| Mish | $x \cdot \tanh(\ln(1 + e^x))$ | — | Self-regularized |
| Snake | $x + \frac{1}{\alpha} \sin^2(\alpha x)$ | $\alpha \in \mathbb{R}^+$ | Periodic activations |

### 4.3 Per-Neuron Parameter Learning

Each activation function with learnable parameters updates via simple gradient descent:

For PReLU:

$$\frac{\partial \mathcal{L}}{\partial \alpha_i} = \sum_{j} \frac{\partial \mathcal{L}}{\partial a_j} \cdot \frac{\partial a_j}{\partial \alpha_i} = \sum_{j \in \text{out}(i)} w_{ji} \cdot \delta_j \cdot \begin{cases} x_i & \text{if } \alpha_i x_i < x_i \\ 0 & \text{otherwise} \end{cases}$$

$$\alpha_i^{(t+1)} = \alpha_i^{(t)} - \eta_\alpha \cdot \frac{\partial \mathcal{L}}{\partial \alpha_i}$$

### 4.4 Integration

```rust
pub enum ActivationFn {
    Tanh,
    PReLU { alpha: f32 },
    Swish { beta: f32 },
    GELU,
    AdaptiveTanh { gamma: f32 },
    Softplus,
    Mish,
    Snake { alpha: f32 },
}

pub struct ActivationConfig {
    pub per_neuron: Vec<ActivationFn>,
    pub shared: Option<ActivationFn>,  // fallback if per_neuron not set
}
```

---

## 5. Attention-Like Routing

### 5.1 Motivation

Current weight updates are purely Hebbian (correlation-based). Attention mechanisms allow the network to dynamically route information based on *content*, not just pairwise correlation.

### 5.2 Hedged Attention — Lightweight Attention Over Synapse Graph

For a neuron $i$ receiving input from its $K$ presynaptic partners $\mathcal{N}_i$, compute an attention distribution over the incoming signals before aggregation:

$$e_{ij} = \mathbf{v}^\top \tanh(\mathbf{W}_q a_i + \mathbf{W}_k a_j) \quad \text{for } j \in \mathcal{N}_i$$

$$\alpha_{ij} = \frac{\exp(e_{ij})}{\sum_{k \in \mathcal{N}_i} \exp(e_{ik})}$$

The aggregated signal (replacing the simple weighted sum) is:

$$x_i = \sum_{j \in \mathcal{N}_i} \alpha_{ij} \cdot w_{ij} \cdot a_j$$

### 5.3 Gating Mechanism

A simpler alternative to full attention: compute a per-synapse gate $g_{ij} \in [0, 1]$ that modulates whether a synapse passes its signal:

$$g_{ij} = \sigma(\mathbf{w}_g^\top [a_i; a_j] + b_g)$$

$$x_i = \sum_{j \in \mathcal{N}_i} g_{ij} \cdot w_{ij} \cdot a_j$$

This is equivalent to dynamic weighted connections where the weight depends on both pre- and post-synaptic activity.

### 5.4 Integration

```rust
pub enum RoutingMechanism {
    /// Standard Hebbian weighted sum (existing)
    HebbianSum,
    /// Attention-weighted sum with learned queries/keys
    HedgedAttention {
        query_dim: usize,
        key_dim: usize,
        // per-neuron learned projection vectors
    },
    /// Sigmoid gating of each synapse
    GatedRouting,
    /// Mixture of experts: route to top-K weighted synapses
    MoERouting { top_k: usize },
}
```

### 5.5 Complexity

| Mechanism | Time | Memory | Communication overhead |
|-----------|------|--------|----------------------|
| Hebbian sum | $O(dm^2)$ | $O(dm^2)$ | None |
| Hedged attention | $O(dm^2 + mk)$ | $O(dm^2 + mk)$ | $O(K_{\text{attn}})$ per gossip |
| Gated routing | $O(dm^2)$ | $O(3dm^2)$ | None (gates local) |
| MoE routing | $O(dm^2 \log K)$ | $O(dm^2)$ | $O(K_{\text{moe}})$ per gossip |

---

## 6. Online Continual Learning

### 6.1 Motivation

The network operates in an online streaming setting — observations arrive one at a time. Without continual learning mechanisms, the network overwrites previously learned patterns (catastrophic forgetting).

### 6.2 Elastic Weight Consolidation (EWC)

Add a quadratic penalty to the learning update that protects important weights:

$$w_{ij}^{(t+1)} = w_{ij}^{(t)} + \eta \cdot a_i^{(t)} \cdot a_j^{(t)} - \lambda \cdot w_{ij}^{(t)} - \gamma \cdot F_{ij} \cdot (w_{ij}^{(t)} - w_{ij}^*)$$

where:
- $F_{ij}$ is the Fisher information matrix diagonal element for synapse $(i,j)$
- $w_{ij}^*$ is the weight value after the previous task
- $\gamma$ is the EWC importance multiplier

**Fisher information estimation:**

$$F_{ij} = \mathbb{E}\left[\left(\frac{\partial \mathcal{L}}{\partial w_{ij}}\right)^2\right]$$

In the Hebbian setting, the "loss" is the prediction error $\gamma_t = |\hat{o}_t - o_t|$, so:

$$F_{ij}^{(t+1)} = (1 - \rho_F) F_{ij}^{(t)} + \rho_F \cdot \left(\frac{\partial \gamma_t}{\partial w_{ij}}\right)^2$$

### 6.3 Synaptic Intelligence (SI)

Track the importance of each synapse through its contribution to the total reduction in loss over its lifetime:

$$\Omega_{ij} = \sum_{t} \frac{w_{ij}^{(t+1)} - w_{ij}^{(t)}}{\sqrt{\sum_t (w_{ij}^{(t+1)} - w_{ij}^{(t)})^2}} \cdot \frac{\partial \mathcal{L}}{\partial w} \bigg|_{t}$$

The SI protection term replaces the EWC penalty:

$$w_{ij}^{(t+1)} = w_{ij}^{(t)} + \eta \cdot \Delta w_{ij} - \lambda \cdot w_{ij}^{(t)} - \gamma \cdot \Omega_{ij} \cdot (w_{ij}^{(t)} - w_{ij}^*)$$

### 6.4 Progressive Networks

For each new "task" (identified by a shift in input statistics), allocate a new column of neurons rather than overwriting existing ones. This is a natural extension of the existing neurogenesis mechanism — instead of a single spawn, create a sub-network.

```rust
pub enum ContinualLearning {
    None,                    // standard Hebbian (no protection)
    EWC {
        gamma: f32,         // importance multiplier
        fisher_decay: f32,  // ρ_F for Fisher estimation
    },
    SynapticIntelligence {
        gamma: f32,
    },
    ProgressiveNetwork {
        column_size: usize,
        max_columns: usize,
    },
}
```

---

## 7. Catastrophic Forgetting Benchmarks

### 7.1 Benchmark Protocol

Standard continual learning benchmarks adapted to the neuron-wire setting:

| Benchmark | Description | Metrics |
|-----------|-------------|---------|
| **Permuted MNIST** | MNIST pixels permuted each task; network must learn new permutation without forgetting old ones | Accuracy per task, BWT, FWT |
| **Rotated MNIST** | MNIST digits rotated by random angle per task | Accuracy, forgetting rate |
| **Split CIFAR-10** | CIFAR-10 split into 5 binary tasks (classes 0-1, 2-3, ...) | Accuracy per task, forgetting |
| **CW-10 (Continual Wiki)** | Synthetic streaming tasks with known correlation shifts | Prediction error trajectory |
| **5-Dataset** | Sequential learning on 5 different datasets (MNIST, SVHN, CIFAR-10, etc.) | Cross-dataset transfer |

### 7.2 Metrics

| Metric | Definition | Interpretation |
|--------|-----------|---------------|
| **Accuracy** | $A_t = \frac{1}{t} \sum_{i=1}^t a_{t,i}$ | Average accuracy across all $t$ tasks after learning task $t$ |
| **Backward Transfer (BWT)** | $\frac{1}{t-1} \sum_{i=1}^{t-1} (a_{t,i} - a_{i,i})$ | How much learning task $t$ affects performance on earlier tasks |
| **Forward Transfer (FWT)** | $\frac{1}{t-1} \sum_{i=2}^{t} (a_{i-1,i} - b_i)$ | How much previous tasks help learning new tasks (vs random baseline $b_i$) |
| **Forgetting Rate** | $\frac{1}{t-1} \sum_{i=1}^{t-1} \max_{\tau \in \{i,\dots,t-1\}} (a_{\tau,i} - a_{t,i})$ | Maximum drop in accuracy for each task across all later tasks |
| **Stability** | $\|a_{t,t} - a_{t-1,t-1}\|$ | How much the current task's accuracy changes when learning it |

### 7.3 Reporting

Every benchmark reports the full statistical snapshot per metric:

> BWT: $\bar{x} = -0.12$  median $= -0.09$  $s^2 = 0.04$  CI$_{95\%} = [-0.15, -0.09]$  $d = 0.84$

---

## 8. Knowledge Distillation

### 8.1 Motivation

In a decentralized setting, different nodes may have different model capacities or data distributions. Distillation allows a larger "teacher" node to transfer knowledge to smaller "student" nodes without sharing raw data.

### 8.2 Local Distillation via Gossip

During gossip exchange, a node sends *soft targets* (activation distributions) rather than raw weights:

$$p_i^{(t)} = \frac{\exp(a_i^{(t)} / \tau)}{\sum_j \exp(a_j^{(t)} / \tau)}$$

where $\tau$ is the temperature parameter. Higher $\tau$ produces softer probability distributions that carry dark knowledge.

The receiving node minimizes the KL divergence between its own soft targets and the teacher's:

$$\mathcal{L}_{\text{distill}} = \tau^2 \cdot D_{KL}(p_{\text{teacher}} \parallel p_{\text{student}}) + \alpha \cdot \mathcal{L}_{\text{Hebbian}}$$

where $\alpha$ balances distillation vs. local learning.

### 8.3 Distributed Distillation Protocol

```
Student Node S                           Teacher Node T
     │                                       │
     │  GOSSIP frame (activations + τ)        │
     │──────────────────────────────────────>│
     │                                       │
     │  GOSSIP response (soft targets p_T)   │
     │<──────────────────────────────────────│
     │                                       │
     │  S computes: D_KL(p_T || p_S)         │
     │  Updates weights via ∇_w D_KL         │
     │                                       │
```

### 8.4 Integration

```rust
pub struct DistillationConfig {
    pub enabled: bool,
    pub temperature: f32,        // τ (default: 2.0)
    pub alpha: f32,              // distillation vs Hebbian balance
    pub teacher_select: PeerSelection,
}
```

---

## 9. Distributed Replay Buffers

### 9.1 Motivation

Replay (experience replay in RL / episodic memory in continual learning) buffers past observations for interleaved training, mitigating catastrophic forgetting and enabling offline learning.

### 9.2 Local Replay Buffer

Each node maintains a circular buffer of $(o_t, a_t, \hat{o}_t, \gamma_t)$ tuples:

$$\mathcal{B}_i = \{(o_\tau, a_\tau, \hat{o}_\tau, \gamma_\tau) : \tau \in [t-R+1, t]\}$$

where $R$ is the buffer capacity (default $10^4$ entries).

At each tick, with probability $p_{\text{replay}}$, sample a mini-batch from $\mathcal{B}_i$ and perform a standard Hebbian update on the replayed activations:

$$w_{ij}^{(t+1)} = w_{ij}^{(t)} + \eta \cdot \mathbb{E}_{(o,a,\hat{o},\gamma) \sim \mathcal{B}_i}[a_i \cdot a_j] - \lambda \cdot w_{ij}^{(t)}$$

### 9.3 Distributed Replay via Gossip

Nodes share replay buffer samples during gossip. Each gossip frame includes a small set of $(o, a, \hat{o})$ tuples from the sender's buffer. The receiver samples from both local and remote buffers:

$$\nabla w \propto \mathbb{E}_{\mathcal{B}_{\text{local}}}[\nabla w] + \beta \cdot \mathbb{E}_{\mathcal{B}_{\text{remote}}}[\nabla w]$$

where $\beta \in [0, 1]$ controls the influence of remote experience.

### 9.4 Integration

```rust
pub struct ReplayBuffer {
    pub capacity: usize,
    pub batch_size: usize,
    pub replay_probability: f32,
    // circular buffer storage
    observations: Vec<Observation>,
    activations: Vec<ActivationVector>,
    predictions: Vec<Prediction>,
    position: usize,
    count: usize,
}

pub struct DistributedReplay {
    pub local: ReplayBuffer,
    pub remote_beta: f32,  // weight of remote samples
    pub share_interval: u64,  // how often to share samples
}
```

---

## 10. Local Memory Modules

### 10.1 Motivation

Beyond replay, differentiable memory modules allow the network to store and retrieve structured information — key-value pairs, associative memories, or temporal sequences.

### 10.2 Differentiable Neural Memory

A sparse key-value memory:

$$M = \{(k_j, v_j) : j = 1 \dots N\}$$

**Write** (during observation):

$$k_t = \text{Enc}(a_t) \quad v_t = \text{Enc}(o_t)$$

Insert $(k_t, v_t)$ into memory (evict least-recently-used if full).

**Read** (during prediction):

$$s_j = \frac{k_t \cdot k_j}{\|k_t\| \cdot \|k_j\|} \quad \text{(cosine similarity)}$$

$$\alpha_j = \frac{\exp(s_j / \tau)}{\sum_{j'} \exp(s_{j'} / \tau)} \quad \text{(attention over memory)}$$

$$v_{\text{retrieved}} = \sum_j \alpha_j v_j$$

The retrieved memory augments the prediction: $\hat{o}_t = f(w_{\text{readout}} \cdot a_t, v_{\text{retrieved}})$.

### 10.3 Integration

```rust
pub struct MemoryModule {
    pub capacity: usize,
    pub key_dim: usize,
    pub value_dim: usize,
    pub temperature: f32,
    // storage
    keys: Vec<Vec<f32>>,
    values: Vec<Vec<f32>>,
    usage: Vec<u64>,  // recency counter for LRU eviction
}
```

---

## 11. Prediction Uncertainty & Bayesian Confidence

### 11.1 Motivation

The existing system outputs a single point prediction $\hat{o}_t$. Without uncertainty quantification, the network cannot distinguish between "I am confident" and "I am guessing."

### 11.2 Bayesian Prediction

Treat each weight as a Gaussian random variable:

$$w_{ij} \sim \mathcal{N}(\mu_{ij}, \sigma_{ij}^2)$$

The forward pass propagates uncertainty:

$$\mu_{x_i} = \sum_j \mu_{ij} \cdot a_j$$

$$\sigma_{x_i}^2 = \sum_j (\sigma_{ij}^2 \cdot a_j^2 + \mu_{ij}^2 \cdot \sigma_{a_j}^2 + \sigma_{ij}^2 \cdot \sigma_{a_j}^2)$$

The output prediction is a Gaussian:

$$\hat{o} \sim \mathcal{N}(\mu_{\hat{o}}, \sigma_{\hat{o}}^2)$$

**Confidence score:** $c_t = 1 / \sigma_{\hat{o}}^2$ (precision). High precision = high confidence.

### 11.3 Epistemic vs. Aleatoric Uncertainty

| Type | Source | Decreases with | Detection |
|------|--------|---------------|-----------|
| Aleatoric | Observation noise | More data (same noise level) | High $\sigma$ independent of training |
| Epistemic | Model uncertainty | More training (weights converge) | High $\sigma$ that decreases with training |

In practice: maintain an ensemble of $E$ weight matrices $\{W^{(1)}, \dots, W^{(E)}\}$ with different random seeds. The variance across ensemble members is epistemic uncertainty; the average variance within members is aleatoric.

### 11.4 Integration

```rust
pub struct BayesianConfig {
    pub enabled: bool,
    pub method: BayesianMethod,
    pub ensemble_size: usize,
}

pub enum BayesianMethod {
    /// Single weight = Gaussian (μ, σ)
    BayesByBackprop,
    /// Deep ensemble: E forward passes
    DeepEnsemble,
    /// Monte Carlo dropout: stochastic forward pass
    MCDropout { dropout_prob: f32 },
}

pub struct Prediction {
    pub mean: f32,
    pub variance: f32,        // total uncertainty
    pub epistemic: f32,       // model uncertainty
    pub aleatoric: f32,       // irreducible noise
    pub confidence: f32,      // 1 / variance
}
```

---

## 12. Curiosity Scheduling & Intrinsic Motivation

### 12.1 Motivation

The current system uses prediction error $\gamma_t$ as the sole driver of neurogenesis and learning. Curiosity augments this with *intrinsic* rewards that drive exploration.

### 12.2 Intrinsic Motivation Module

$$r_t^{\text{intrinsic}} = \underbrace{\gamma_t}_{\text{prediction error}} + \underbrace{\beta_{\text{count}} \cdot \frac{1}{\sqrt{N(o_t) + 1}}}_{\text{count-based novelty}} + \underbrace{\beta_{\text{info}} \cdot I(o_t; \theta_t)}_{\text{information gain}}$$

where:
- $N(o_t)$ is the visit count for observation $o_t$ (discretized/hashed)
- $I(o_t; \theta_t)$ is the information gain about the weights from observing $o_t$
- $\beta_{\text{count}}, \beta_{\text{info}}$ are hyperparameters

### 12.3 Curiosity Scheduling

The exploration-exploitation balance evolves over time:

$$\beta(t) = \beta_0 \cdot \exp(-t / \tau_{\text{curiosity}}) + \beta_{\infty}$$

This creates a natural curriculum: early in training, curiosity dominates (exploration), later it anneals to a baseline (exploitation).

### 12.4 Curiosity Bonus in Neurogenesis

Augment the spawn probability (§3.1 of FORMAL_MODEL):

$$P(\text{spawn}) = 1 - \exp(-\beta \cdot (\Gamma_t + r_t^{\text{intrinsic}} - \sigma)_+)$$

### 12.5 Integration

```rust
pub struct CuriosityModule {
    pub count_beta: f32,       // β_count
    pub info_beta: f32,        // β_info
    pub schedule: CuriositySchedule,
    // state
    visit_counts: HashMap<ObservationHash, u64>,
    info_gain_estimate: f32,
}

pub enum CuriositySchedule {
    Constant(f32),              // fixed β
    Exponential { beta_0: f32, tau: f32, beta_inf: f32 },
    CosineAnnealing { t_max: u64, eta_min: f32 },
    Adaptive,                   // β decreases when γ_t < threshold for T ticks
}
```

---

## 13. Meta-Learning

### 13.1 Motivation

Instead of hand-tuning hyperparameters ($\eta$, $\lambda$, $\sigma$, $\beta$, etc.), meta-learn them. The network learns how to learn.

### 13.2 Per-Parameter Hyper-Network

For each synapse $(i,j)$, a small hyper-network $h_\phi$ outputs the learning rate and decay:

$$(\eta_{ij}^{(t)}, \lambda_{ij}^{(t)}) = h_\phi(a_i^{(t)}, a_j^{(t)}, w_{ij}^{(t)}, G_{ij}^{(t)})$$

The hyper-network $h_\phi$ is a tiny MLP (2 hidden layers, 32 units) shared across all synapses.

### 13.3 Learned Update Rules (L2L — Learning to Learn)

Replace the fixed Hebbian update with a learned function:

$$\Delta w_{ij}^{(t)} = g_\theta(a_i^{(t)}, a_j^{(t)}, w_{ij}^{(t)}, h_{ij}^{(t)})$$

where $h_{ij}$ is a hidden state that carries gradient information across ticks (similar to an LSTM optimizer). The meta-parameters $\theta$ are updated via:

$$\theta^* = \arg\min_\theta \mathbb{E}_{\text{task}}[\mathcal{L}_{\text{meta}}(w(T; \theta))]$$

In practice, this uses truncated backprop through time (BPTT) over a window of $T_{\text{meta}} = 100$ ticks.

### 13.4 Integration

```rust
pub enum MetaLearning {
    None,
    HyperNet {
        hidden_dim: usize,
        output_dim: usize,
    },
    LearnedOptimizer {
        hidden_dim: usize,
        unroll_steps: u64,
        meta_lr: f32,
    },
}
```

---

## 14. Federated Baseline

### 14.1 Overview

Standard Federated Averaging (FedAvg) for comparison against decentralized Hebbian learning. A central server coordinates $n$ clients performing local SGD.

### 14.2 Algorithm

```
Algorithm: FedAvg
For each round r = 1, 2, ..., R:
  1. Server selects fraction C of clients (C = 0.1)
  2. Each selected client k:
     a. Receives global model w_r from server
     b. Performs E epochs of local SGD on local data
     c. Sends Δw_k = w_k - w_r back to server
  3. Server aggregates: w_{r+1} = w_r + η·Σ(p_k · Δw_k)
```

### 14.3 Communication Cost

Per round: $2C \cdot |w|$ (download + upload). Compare with NWP gossip: $O(g \cdot K_{\text{syn}})$ per tick.

### 14.4 Implementation

```python
# baselines/federated.py
class FederatedBaseline:
    def __init__(self, n_clients=50, fraction=0.1, local_epochs=5):
        self.global_model = ...
        self.clients = [Client(data_i) for i in range(n_clients)]
    
    def round(self):
        selected = random.sample(self.clients, int(self.fraction * len(self.clients)))
        deltas = [client.train(self.global_model, self.local_epochs) for client in selected]
        self.global_model += self.lr * mean(deltas)
        return self.evaluate()
```

---

## 15. Decentralized SGD Baseline

### 15.1 Overview

Nodes perform SGD on local data and average weights with their graph neighbors. The communication graph is a random regular graph (degree $d = \log n$).

### 15.2 Algorithm

```
Algorithm: Decentralized SGD
For each tick t = 1, 2, ..., T:
  1. Each node i:
     a. Computes local gradient g_i(t) on mini-batch
     b. Sends w_i(t) to neighbors j ∈ N(i)
     c. Receives w_j(t) from neighbors
     d. Averages: w_i(t+1) = Σ_{j∈N(i)∪{i}} w_j(t) / (|N(i)|+1)
     e. Applies gradient: w_i(t+1) -= η · g_i(t)
```

### 15.3 Comparison with NWP

| Aspect | Decentralized SGD | NWP Hebbian |
|--------|------------------|-------------|
| Update rule | Gradient descent on loss | Hebbian STDP |
| Communication | Full model averaging | Gossip of top-K weights |
| Synchronization | Averaging creates consensus | No consensus needed |
| Convergence rate | $O(1/\sqrt{nT})$ (smooth) | $O((1-\lambda)^T)$ (geometric) |
| Fault tolerance | Low (averaging requires all) | High (gossip is best-effort) |

---

## 16. Parameter Server Baseline

### 16.1 Overview

A synchronous parameter server (PS) architecture for comparison.

### 16.2 Architecture

```
Worker 1 ──push/pull──> Parameter Server <──push/pull── Worker 2
Worker 3 ──push/pull──>                    <──push/pull── Worker 4
```

Workers compute gradients locally, push to the server. Server aggregates and sends updated parameters back. Uses bounded asynchronous delays (bounded-async PS) for performance.

---

## 17. Ray Baseline

### 17.1 Overview

Ray (RLlib/ Ray Tune) distributed reinforcement learning baseline. Demonstrates the performance of centralized distributed training with a mature framework.

### 17.2 Integration

```python
# baselines/ray_baseline.py
import ray
from ray import tune

@ray.remote
class RayWorker:
    def compute_gradient(self, params):
        # local computation
        return grad

ray.init()
workers = [RayWorker.remote() for _ in range(50)]
```

---

## 18. Horovod Baseline

### 18.1 Overview

Horovod — all-reduce based distributed training using ring-reduce for gradient synchronization.

### 18.2 Comparison

| Metric | Horovod | NWP |
|--------|---------|-----|
| Synchronization | All-reduce (blocking) | Gossip (async) |
| Scalability | $O(n)$ communication per step | $O(g)$ per node (constant) |
| Fault tolerance | Low (all ranks required) | High (best-effort) |
| Bandwidth at $n=50$ | $\sim 50 \times |w|$ per step | $\sim 3 \times K_{\text{syn}}$ per tick |

---

## 19. Bittensor Baseline

### 19.1 Overview

Bittensor — a peer-to-peer network where miners serve model outputs and validators stake TAO to rank them. NWP differs fundamentally in that every node is both a learner and a router.

### 19.2 Key Differences

| Aspect | Bittensor | NWP |
|--------|-----------|-----|
| Topology | Star (miners ↔ validators) | Mesh (P2P Kademlia) |
| Incentive | TAO token rewards | None (cooperative) |
| Learning | Miners train, validators rank | Every node learns independently |
| Update mechanism | Gradients via chain | Hebbian gossip |
| Identity | Cryptographic (hotkey) | NodeId (currently unauthenticated) |

---

## 20. Benchmark Framework

### 20.1 Running Benchmarks

```bash
# Catastrophic forgetting benchmark
cargo run --release --example continual_learning -- \
    --benchmark permuted_mnist \
    --tasks 10 \
    --epochs-per-task 5 \
    --continual-learning ewc \
    --gamma 100.0

# Distributed baseline comparison
python baselines/run_comparison.py \
    --framework nwp,federated,decentralized,ps,horovod \
    --n-nodes 50 \
    --epochs 100 \
    --output benchmarks/comparison.csv
```

### 20.2 Reporting Format

Every benchmark outputs a standardized CSV:

```csv
benchmark,framework,metric,mean,median,variance,ci95_low,ci95_high,cohens_d,p_value,power
permuted_mnist,nwp,accuracy,0.72,0.74,0.03,0.70,0.74,1.2,0.001,0.99
permuted_mnist,federated,accuracy,0.85,0.86,0.01,0.84,0.86,2.1,<0.001,1.0
```

### 20.3 Comparison Metrics Per Feature

Every ML feature is compared against a baseline using:

| Metric | Baseline | Interpretation |
|--------|----------|---------------|
| Prediction error | Fixed LR ($\eta=0.01$) | Lower = better adaptation |
| Convergence time (ticks) | Fixed LR | Fewer ticks = faster learning |
| Forgetting (BWT) | No continual learning | Less negative = less forgetting |
| Bandwidth (bytes/tick) | No compression | Lower = better compression |
| Memory (bytes) | Dense storage | Lower = sparser representation |
| Uncertainty calibration | Point prediction | Better = well-calibrated confidence |

---

## Baseline Scripts

### Training loop comparison (pseudo-code)

```python
# baselines/comparison_framework.py
class ComparisonFramework:
    """
    Unified framework for comparing NWP against established baselines.
    All baselines receive the same synthetic data stream and report
    the same metrics.
    """
    def __init__(self, n_nodes=50, n_features=100, n_classes=10):
        self.data = self._generate_synthetic()
        self.baselines = {
            'nwp': NWPAdapter(n_nodes),
            'federated': FederatedBaseline(n_nodes),
            'decentralized': DecentralizedSGD(n_nodes, degree=5),
            'ps': ParameterServer(n_nodes),
            'ray': RayWrapper(n_nodes),
            'horovod': HorovodWrapper(n_nodes),
        }
    
    def evaluate(self, metrics=['accuracy', 'bandwidth', 'convergence']):
        results = {}
        for name, bl in self.baselines.items():
            bl.train(self.data)
            results[name] = bl.report(metrics)
        return results
```

See `baselines/` directory for full implementations.

---

## References

- Kingma & Ba, *Adam: A Method for Stochastic Optimization*, ICLR 2015.
- Duchi et al., *Adaptive Subgradient Methods for Online Learning and Stochastic Optimization*, JMLR 2011.
- Hinton et al., *Distilling the Knowledge in a Neural Network*, NeurIPS 2014 Deep Learning Workshop.
- Kirkpatrick et al., *Overcoming catastrophic forgetting in neural networks*, PNAS 2017.
- Zenke et al., *A Simple Approach to Continual Learning by Synaptic Intelligence*, PNAS 2017.
- Rusu et al., *Progressive Neural Networks*, arXiv 2016.
- Vaswani et al., *Attention Is All You Need*, NeurIPS 2017.
- Graves et al., *Neural Turing Machines*, arXiv 2014.
- Burda et al., *Large-Scale Study of Curiosity-Driven Learning*, ICLR 2019.
- Andrychowicz et al., *Learning to learn by gradient descent by gradient descent*, NeurIPS 2016.
- McMahan et al., *Communication-Efficient Learning of Deep Networks from Decentralized Data*, AISTATS 2017.
- Lian et al., *Can Decentralized Algorithms Outperform Centralized Algorithms? A Case Study for Decentralized Parallel Stochastic Gradient Descent*, NeurIPS 2017.
- Li et al., *Scaling Distributed Machine Learning with the Parameter Server*, OSDI 2014.
- Moritz et al., *Ray: A Distributed Framework for Emerging AI Applications*, OSDI 2018.
- Sergeev & Del Balso, *Horovod: fast and easy distributed deep learning in TensorFlow*, arXiv 2018.
- [FORMAL_MODEL.md](FORMAL_MODEL.md) — Formal mathematical model, convergence proofs, complexity analysis
- [PAPER.md](PAPER.md) — Research paper
- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture
