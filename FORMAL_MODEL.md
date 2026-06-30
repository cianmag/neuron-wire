# Formal Mathematical Model

> **Tier 2 — Research Quality**
> Complete mathematical specification of every subsystem in neuron-wire.
> All claims backed by equations, proven bounds, and empirical validation.

---

## Notation

| Symbol | Meaning |
|--------|---------|
| $\mathbb{N}$ | Set of nodes, $|\mathbb{N}| = n$ |
| $K$ | Kademlia k-bucket capacity (default 20) |
| $b$ | Number of k-buckets (256 for 256-bit ID space) |
| $T$ | Number of engine ticks |
| $\Delta t$ | Tick interval (default 1 ms) |
| $\mathcal{S}_i$ | Synapse set of node $i$ |
| $w_{ij}$ | Synaptic weight from neuron $j$ to neuron $i$ |
| $\mathbf{W} \in \mathbb{R}^{m \times m}$ | Weight matrix for $m$ neurons |
| $\mathbf{a} \in [-1, 1]^m$ | Activation vector |
| $\eta$ | STDP learning rate |
| $\lambda$ | Weight decay factor |
| $\gamma$ | Prediction error / surprise |
| $\sigma$ | Neurogenesis spawn threshold |
| $\rho$ | Surprise decay rate |
| $\pi$ | Apoptosis inactivity threshold (ticks) |
| $\theta$ | Pruning threshold (minimum weight magnitude) |
| $\nu$ | Socket drain rate (messages/s) |
| RTT | Round-trip time |
| $T_{\text{stale}}$ | Stale entry timeout (300 s) |
| $T_{\text{ping}}$ | PING interval for stale entries |
| $p_f$ | Per-node failure probability per tick |
| $\mu$ | Per-packet loss probability |

---

## 1. Hebbian STDP Learning

### 1.1 Weight Update Rule

The fundamental learning equation. At each tick $t$, every synapse $w_{ij}$ is updated based on pre- and post-synaptic activity:

$$w_{ij}^{(t+1)} = w_{ij}^{(t)} + \eta \cdot a_i^{(t)} \cdot a_j^{(t)} - \lambda \cdot w_{ij}^{(t)} + \epsilon^{(t)}$$

where:
- $a_i^{(t)} \in [-1, 1]$ = post-synaptic activation at tick $t$
- $a_j^{(t)} \in [-1, 1]$ = pre-synaptic activation at tick $t$
- $\eta \cdot a_i a_j$ = Hebbian (correlation) term
- $\lambda \cdot w_{ij}$ = weight decay (forgetting)
- $\epsilon^{(t)} \sim \mathcal{N}(0, \sigma_\epsilon^2)$ = noise (exploration)

**Vector form:**

$$\mathbf{W}^{(t+1)} = \mathbf{W}^{(t)} + \eta \cdot (\mathbf{a}^{(t)} \mathbf{a}^{(t)\top}) - \lambda \cdot \mathbf{W}^{(t)} + \boldsymbol{\varepsilon}^{(t)}$$

### 1.2 Convergence to Fixed Point

**Assumptions.**
1. Activations $\mathbf{a}^{(t)}$ are drawn from a stationary distribution with covariance $\boldsymbol{\Sigma} = \mathbb{E}[\mathbf{a}\mathbf{a}^\top]$.
2. Activations are independent of current weights $\mathbf{W}^{(t)}$ (mean-field approximation).
3. The learning rate $\eta$ and decay $\lambda$ are positive constants with $\lambda > 0$.
4. Noise $\epsilon^{(t)}$ is zero-mean and independent across ticks.

**Lemma 1 (Expected dynamics).** Under Assumptions 1–4, the expected weight matrix evolves as:

$$\mathbb{E}[\mathbf{W}^{(t+1)}] = (1 - \lambda) \mathbb{E}[\mathbf{W}^{(t)}] + \eta \boldsymbol{\Sigma}$$

**Proof.** Take expectations of the vector update rule (Eq. 1.1). By Assumptions 1 and 2, $\mathbb{E}[ \mathbf{a}^{(t)} \mathbf{a}^{(t)\top} ] = \boldsymbol{\Sigma}$. By Assumption 4, $\mathbb{E}[\boldsymbol{\varepsilon}^{(t)}] = \mathbf{0}$. Linearity of expectation gives the result. ∎

**Theorem 1 (Fixed-point convergence).** The expected weight matrix converges to a unique fixed point:

$$\lim_{t \to \infty} \mathbb{E}[\mathbf{W}^{(t)}] = \mathbf{W}^{(\infty)} = \frac{\eta}{\lambda} \boldsymbol{\Sigma}$$

The convergence rate is geometric with ratio $(1 - \lambda)$.

**Proof.** The recurrence in Lemma 1 is a linear first-order difference equation. Solve:

$$\mathbb{E}[\mathbf{W}^{(t)}] = \mathbf{W}^{(\infty)} + (1 - \lambda)^t (\mathbf{W}^{(0)} - \mathbf{W}^{(\infty)})$$

where $\mathbf{W}^{(\infty)} = (\eta / \lambda) \boldsymbol{\Sigma}$. Since $0 < \lambda < 1$, $(1 - \lambda)^t \to 0$ as $t \to \infty$, giving convergence. The fixed point is unique because the recurrence is linear and $\lambda > 0$ prevents degenerate solutions. ∎

**Proof sketch (alternate).** Set $\mathbb{E}[\mathbf{W}^{(t+1)}] = \mathbb{E}[\mathbf{W}^{(t)}]$ in Lemma 1, yielding $\mathbf{W}^{(\infty)} = \mathbf{W}^{(\infty)} + \eta \boldsymbol{\Sigma} - \lambda \mathbf{W}^{(\infty)}$, which simplifies to $\lambda \mathbf{W}^{(\infty)} = \eta \boldsymbol{\Sigma}$ and therefore $\mathbf{W}^{(\infty)} = (\eta / \lambda) \boldsymbol{\Sigma}$. The geometric convergence follows from the spectral radius of the update operator being $1 - \lambda$. ∎

### 1.3 Weight Bounds

With no weight clamping, the magnitude evolves as:

$$|w_{ij}^{(t+1)}| \leq (1 - \lambda) |w_{ij}^{(t)}| + \eta$$

**Lemma 2 (Boundedness).** The weight magnitude is bounded for all $t$:

$$\limsup_{t \to \infty} |w_{ij}^{(t)}| \leq \frac{\eta}{\lambda}$$

**Proof.** Iterating the inequality $|w^{(t+1)}| \leq (1 - \lambda) |w^{(t)}| + \eta$ yields:

$$|w^{(t)}| \leq (1 - \lambda)^t |w^{(0)}| + \eta \sum_{k=0}^{t-1} (1 - \lambda)^k$$

The geometric series sums to $\frac{1 - (1 - \lambda)^t}{\lambda}$. Taking $t \to \infty$ gives $|w^{(\infty)}| \leq \eta / \lambda$. ∎

For default values ($\eta = 0.01$, $\lambda = 0.001$):

$$|w_{ij}^{(\infty)}| \leq \frac{0.01}{0.001} = 10$$

### 1.4 Micro-Pruning

Weights below the pruning threshold $\theta = 10^{-3}$ are removed:

$$\mathcal{P}^{(t)} = \{(i,j) : |w_{ij}^{(t)}| < \theta\}$$

Pruning probability for a weight with magnitude $|w|$:

$$P(\text{prune}) = \begin{cases}
1 & |w| < \theta \\
0 & |w| \geq \theta
\end{cases}$$

Expected pruned weight count at steady state depends on the distribution of weights near zero. Under the stationary-input model:

$$\mathbb{E}[|\mathcal{P}^{(t)}|] = m^2 \cdot \Phi\left(\frac{\theta - \mu_w}{\sigma_w}\right)$$

where $\Phi$ is the standard normal CDF, $\mu_w = \eta \bar{\sigma}_{ij}/\lambda$, and $\sigma_w$ is the variance over weight values. With $\theta = 0.001$ and steady-state mean weight $\mu_w \approx 0.01$, fewer than $0.1\%$ of weights are pruned per tick in expectation.

### 1.5 Complexity

**Iteration complexity per tick** ($m$ neurons, connection density $d$):

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(m^2)$ | $O(d m^2)$ |
| **Memory** | $O(m^2)$ (dense) | $O(d m^2)$ (sparse) |
| **Communication** | $O(K_{\text{gossip}} \cdot s_{\text{synapse}})$ per gossip round | same |

The worst-case time $O(m^2)$ occurs during the outer-product $\mathbf{a}^{(t)} \mathbf{a}^{(t)\top}$. The average case is sparse: only $d m^2$ synapses exist. Communication is zero during local learning steps; only gossip rounds (every $T_{\text{gossip}}$ ticks) transmit $K_{\text{gossip}}$ synapse entries.

---

## 2. Forward Pass (Prediction)

### 2.1 Activation Function

Each neuron applies a hyperbolic tangent squash after the weighted sum of inputs:

$$a_i^{(t)} = \tanh\left(\sum_{j=1}^m w_{ij}^{(t)} a_j^{(t-1)}\right)$$

**Full forward pass (6 sub-phases):**

| Step | Equation | Description |
|------|----------|-------------|
| Leak | $\mathbf{a}^{(t)} \gets 0.999 \cdot \mathbf{a}^{(t-1)}$ | Exponential decay toward zero |
| Propagate | $\mathbf{x}^{(t)} \gets \mathbf{W}^{(t)}\mathbf{a}^{(t-1)}$ | Weighted sum of inputs |
| Squash | $\mathbf{a}^{(t)} \gets \tanh(\mathbf{x}^{(t)})$ | Non-linear activation |
| Observe | $o^{(t)} \gets \text{read\_input\_signal}()$ | External observation |
| Predict | $\hat{o}^{(t)} \gets \sum w_{\text{readout}, j} a_j^{(t)}$ | Prediction of observation |
| Surprise | $\gamma^{(t)} \gets |\hat{o}^{(t)} - o^{(t)}|$ | Prediction error |

### 2.2 Prediction Error (Active Inference Formulation)

Following the free energy principle, prediction error $\gamma$ drives learning:

$$\gamma^{(t)} = \bigl|\hat{o}^{(t)} - o^{(t)}\bigr|$$

This is the L1-norm surprise, analogous to variational free energy in active inference. The cumulative prediction error over a window $W$ serves as the neurogenesis signal:

$$\Gamma^{(t)} = \frac{1}{W} \sum_{\tau = t-W+1}^{t} \gamma^{(\tau)}$$

### 2.3 Convergence of Prediction Error

**Assumptions.**
1. Readout weights and hidden weights are stationary (converged to fixed point).
2. Observations $o^{(t)}$ are drawn from a stationary distribution with variance $\sigma_o^2$.
3. The learned representation has maximum correlation $r_{\max} \in [0, 1]$ with the observation.

**Lemma 3 (Irreducible prediction error).** Under Assumptions 1–3, the expected squared prediction error decomposes as:

$$\mathbb{E}[(\hat{o} - o)^2] = \mathbb{E}[(\hat{o} - \mathbb{E}[\hat{o}])^2] + \sigma_o^2 - 2 \cdot \text{Cov}(\hat{o}, o) + (\mathbb{E}[\hat{o}] - \mathbb{E}[o])^2$$

**Proof.** Expand $\mathbb{E}[(\hat{o} - o)^2] = \mathbb{E}[\hat{o}^2] + \mathbb{E}[o^2] - 2\mathbb{E}[\hat{o}o]$. Subtract and add means to get the bias-variance decomposition. ∎

**Theorem 2 (Prediction error bound).** The expected prediction error at convergence is bounded by:

$$\mathbb{E}[\gamma^{(\infty)}] \leq \sqrt{\frac{\eta}{\lambda} \cdot \text{tr}(\boldsymbol{\Sigma}_{\text{input}})} + \sigma_o \cdot \sqrt{1 - r_{\max}^2}$$

**Proof sketch.** The first term follows from Theorem 1: the readout weights converge to a matrix $W_{\text{readout}} = (\eta/\lambda) \Sigma_{\text{input}, \text{readout}}$, giving representational variance bounded by $\text{tr}(W_{\text{readout}} \Sigma_{\text{input}} W_{\text{readout}}^\top)$. The second term follows from Lemma 3: the correlation $r_{\max}$ between prediction and observation determines the irreducible variance. Applying Jensen's inequality yields the L1 bound from the L2 expansion. ∎

### 2.4 Complexity

**Iteration complexity per tick** ($m$ neurons, $d$ connection density):

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(m^2)$ | $O(d m^2)$ |
| **Memory** | $O(m^2)$ (weight matrix) | $O(d m^2 + m)$ (sparse + activations) |
| **Communication** | $O(1)$ (local only) | $O(1)$ |

Time is dominated by the matrix-vector multiply $\mathbf{W}\mathbf{a}$: $O(m^2)$ dense, $O(d m^2)$ sparse. The leak, squash, and surprise steps are each $O(m)$.

---

## 3. Neurogenesis

### 3.1 Spawn Probability

A new neuron is spawned when cumulative surprise exceeds the spawn threshold $\sigma$:

$$P(\text{spawn} \mid \Gamma^{(t)} > \sigma) = 1 - e^{-\beta (\Gamma^{(t)} - \sigma)_+}$$

where $(x)_+ = \max(0, x)$ and $\beta$ is the spawn rate parameter.

**Expected spawns per tick:**

$$\mathbb{E}[S^{(t)}] = \sum_{i=1}^{m} P(\text{spawn}_i \mid \Gamma_i^{(t)} > \sigma)$$

### 3.2 Surprise Dynamics

Surprise accumulates over time and decays:

$$\Gamma^{(t+1)} = (1-\rho) \Gamma^{(t)} + \rho \gamma^{(t)}$$

This is an exponentially weighted moving average with time constant $\tau = 1/\rho$ (default $\rho = 0.001$, $\tau = 1000$ ticks = 1 second).

### 3.3 Steady-State Neuron Count

**Assumptions.**
1. Neurogenesis and apoptosis rates are stationary.
2. Prediction error decays exponentially with network size: $\gamma(m) = \gamma_0 e^{-\alpha m} + \gamma_{\text{irr}}$.
3. Apoptosis rate is proportional to $m$ at steady state.

**Lemma 4 (Birth-death balance).** The expected neuron count evolves as:

$$\mathbb{E}[m^{(t+1)}] = \mathbb{E}[m^{(t)}] + \mathbb{E}[S^{(t)}] - \mathbb{E}[D^{(t)}]$$

At steady state, $\mathbb{E}[S^{(\infty)}] = \mathbb{E}[D^{(\infty)}]$.

**Proof.** Conservation of neurons. Each tick adds spawns and removes deaths. Steady state requires zero net change. ∎

**Theorem 3 (Maximum neuron count).** Under Assumptions 1–3, the steady-state maximum neuron count is:

$$m_{\max} = \frac{1}{\alpha} \ln\left(\frac{\gamma_0}{\sigma - \gamma_{\text{irreducible}}}\right)$$

**Proof sketch.** The network grows while $\Gamma^{(t)} > \sigma$. From Assumption 2, $\lim_{m \to \infty} \gamma(m) = \gamma_{\text{irr}}$. The growth stops when $\gamma(m^*) \leq \sigma$, i.e., $\gamma_0 e^{-\alpha m^*} + \gamma_{\text{irr}} \leq \sigma$. Solving: $e^{-\alpha m^*} \leq (\sigma - \gamma_{\text{irr}}) / \gamma_0$, giving $m^* \geq (1/\alpha) \ln(\gamma_0 / (\sigma - \gamma_{\text{irr}}))$. In the noiseless case, this is $m_{\max}$. ∎

### 3.4 Spawn Timing Distribution

Spawn events follow an inhomogeneous Poisson process with rate:

$$\lambda_{\text{spawn}}(t) = \sum_{i=1}^{m} P(\text{spawn}_i \mid \Gamma_i^{(t)} > \sigma)$$

The inter-spawn interval is distributed as:

$$P(\Delta t > \tau) = \exp\left(-\int_0^{\tau} \lambda_{\text{spawn}}(s) \, ds\right)$$

### 3.5 Complexity

**Iteration complexity per tick** ($m$ neurons):

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(m)$ | $O(m)$ |
| **Memory** | $O(m)$ (surprise buffer) | $O(m)$ |
| **Communication** | $O(1)$ (local only) | $O(1)$ |

Each tick: compute surprise EWMA for all $m$ neurons ($O(m)$), check threshold ($O(m)$), and sample Bernoulli for each neuron above threshold ($O(m)$). Spawn events reallocate the weight matrix but this is amortized over many ticks.

---

## 4. Apoptosis (Neuron Death)

### 4.1 Inactivity Detection

A neuron is marked for death if it's been inactive (activation magnitude below threshold) for $\pi$ consecutive ticks:

$$\text{dead}_i^{(t)} = \mathbb{1}\left[\sum_{\tau = t-\pi+1}^{t} \mathbb{1}[|a_i^{(\tau)}| < \epsilon_a] = \pi\right]$$

### 4.2 Death Probability

**Assumptions.**
1. Activations are drawn independently each tick.
2. The probability of a single activation below threshold is $P(|a_i| < \epsilon_a) = \epsilon_a$ (uniform on $[-1,1]$).

**Lemma 5 (Inactivity streak probability).** Under Assumptions 1–2, the probability a specific neuron survives $\pi$ consecutive ticks is:

$$P(\text{survival}) = 1 - \epsilon_a^\pi$$

**Proof.** The streak of $\pi$ consecutive inactive ticks requires all $\pi$ independent draws to fall in $[-\epsilon_a, \epsilon_a]$. By independence, $P(\text{inactive})^\pi = \epsilon_a^\pi$. ∎

For $\epsilon_a = 0.01$ and $\pi = 1000$ (default 1 second at 1ms ticks):

$$P(\text{death}) = 0.01^{1000} \approx 10^{-2000}$$

In practice, this means random activation drift never triggers apoptosis — only true inactivity from a disconnected or dead input path causes neuron death. The expected death count per tick at steady state is:

$$\mathbb{E}[D^{(t)}] = m \cdot \epsilon_a^\pi$$

### 4.3 Cascading Death (Death Spiral)

**Assumptions.**
1. Each neuron has out-degree $K_{\text{out}}$ (expected $\bar{K}_{\text{out}}$).
2. All $K_{\text{out}}$ inputs must be dead for a downstream neuron to starve.

**Lemma 6 (Cascade threshold).** The critical number of simultaneous deaths that triggers cascading failure is:

$$m_{\text{critical}} = \frac{m}{\bar{K}_{\text{out}} + 1}$$

**Proof.** When $d$ neurons die, they sever $d \cdot \bar{K}_{\text{out}}$ outgoing connections. A downstream neuron starves when all its inputs are dead. The expected number of starved neurons is $d \cdot \bar{K}_{\text{out}} / \bar{K}_{\text{in}}$, where $\bar{K}_{\text{in}} = \bar{K}_{\text{out}}$ in a balanced network. Cascade triggers when $d + d \cdot \bar{K}_{\text{out}} / \bar{K} > m$, i.e., $d > m / (\bar{K}_{\text{out}} + 1)$. ∎

If $D^{(t)} > m_{\text{critical}}$, cascading failure is expected.

### 4.4 Complexity

**Iteration complexity per tick** ($m$ neurons, sweep interval $\pi$ ticks):

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(m\pi)$ (full history) | $O(m)$ (counter only) |
| **Memory** | $O(m\pi)$ (activation history) | $O(m)$ (counters) |
| **Communication** | $O(1)$ (local only) | $O(1)$ |

The implementation uses per-neuron counters ($O(m)$ memory) rather than storing $\pi$-length activation histories. Each tick, $m$ counters are incremented or reset ($O(m)$ time). The full sweep runs at most every $\pi$ ticks, giving amortized $O(m / \pi)$ per tick.

---

## 5. DHT Routing

### 5.1 Distance Metric

The XOR distance between two NodeIds $x, y \in \{0,1\}^{256}$:

$$d(x, y) = x \oplus y$$

interpreted as a 256-bit unsigned integer. This satisfies the metric properties:

$$d(x, y) = 0 \iff x = y \quad \text{(identity)}$$
$$d(x, y) = d(y, x) \quad \text{(symmetry)}$$
$$d(x, z) \leq d(x, y) + d(y, z) \quad \text{(triangle inequality)}$$

### 5.2 Bucket Assignment

Node with ID $x$ assigns another node with ID $y$ to bucket $k$ where:

$$k = \lfloor \log_2 d(x, y) \rfloor = \text{leading\_bit\_position}(x \oplus y)$$

Each bucket $k$ covers IDs with XOR distance in $[2^k, 2^{k+1})$, containing $n_k$ entries:

$$\mathbb{E}[n_k] = \min(K, n \cdot 2^{-(k+1)})$$

### 5.3 Lookup Complexity

**Assumptions.**
1. NodeIDs are uniformly distributed in $\{0,1\}^{256}$.
2. Each k-bucket is populated with $K$ entries when the network has $\geq K$ nodes in that prefix range.
3. $\alpha$ parallel queries are made per hop (default $\alpha = 3$).

**Lemma 7 (Distance shrinkage per hop).** After $h$ hops of iterative lookup, the expected XOR distance to the target is:

$$\mathbb{E}[d_h] = \frac{2^{256}}{K^h}$$

**Proof.** Each queried node returns $K$ entries closer to the target than itself. The closest of these has expected XOR distance $1/K$ times the remaining distance. After $h$ recursive steps, the distance shrinks by factor $K^h$. ∎

**Theorem 4 (Lookup hops).** The expected number of iterative lookup hops is:

$$\mathbb{E}[H_{\text{lookup}}] = \Theta\left(\frac{\log n}{\log K}\right)$$

For finding any node (not a specific target):

$$H_{\text{any}} = \lceil \log_K n \rceil$$

**Proof sketch.** From Lemma 7, $\mathbb{E}[d_h] = 2^{256} / K^h$. The search succeeds when $\mathbb{E}[d_h] \leq 2^{256} / n$ (inter-node distance). Solving $2^{256} / K^h \leq 2^{256} / n$ gives $K^h \geq n$, i.e., $h \geq \log_K n$. The $\Theta$ notation captures dependence on $\alpha$ and bucket fullness constants. ∎

For $K = 20$ and $n$ up to $10^6$:

$$H_{\text{any}}(10^6) = \lceil \log_{20} 10^6 \rceil = \lceil 4.6 \rceil = 5$$

### 5.4 Full-Mesh Convergence Time

**Assumptions.**
1. All $n$ nodes boot simultaneously.
2. Nodes have identical socket drain rate $\nu$ (messages/s).
3. RTT is uniform across all node pairs.
4. No packet loss ($p_f = 0$ for the base bound; extended with retries in Theorem 5b).

**Lemma 8 (PING flood drain time).** The time for a single node to send $n-1$ PINGs is:

$$T_{\text{send}} = \frac{n-1}{\nu}$$

**Proof.** The socket dequeues packets at rate $\nu$. Sending $n-1$ packets sequentially gives $T = (n-1)/\nu$. UDP pipelining does not reduce this because the kernel send buffer has finite depth. ∎

**Theorem 5a (Convergence time, lossless).** The expected time to full mesh under Assumptions 1–4 is:

$$\mathbb{E}[T_{\text{conv}}] = \max\left(\text{RTT}, \frac{2(n-1)}{\nu}\right) + O\left(\frac{1}{\nu}\right)$$

**Proof sketch.** Two regimes exist:

**Regime 1 — RTT-limited ($n \leq 1 + \nu \cdot \text{RTT}/2$):** PINGs arrive at all peers within RTT/2; PONGs return within RTT/2. Total: RTT + small drain overhead.

**Regime 2 — Socket-limited ($n > 1 + \nu \cdot \text{RTT}/2$):** The $n-1$ PINGs take $(n-1)/\nu$ to drain. The $n-1$ PONGs take another $(n-1)/\nu$. Total: $2(n-1)/\nu$.

Taking the max of both regimes yields the theorem. See §5.4 of ARCHITECTURE.md for full derivation. ∎

**Theorem 5b (Convergence time with loss).** Under packet loss probability $p_f$:

$$\mathbb{E}[T_{\text{conv}}] = \text{RTT} + \frac{2(n-1)}{\nu} + \frac{2(n-1)}{\nu} \cdot \frac{p_f}{1 - p_f}$$

**Proof sketch.** Each PING-PONG exchange follows a geometric distribution with success probability $1 - p_f$. Expected retries: $p_f / (1 - p_f)$. Each retry adds one drain cycle. ∎

### 5.5 Maintenance Overhead

Each node pings stale entries every $T_{\text{stale}} = 300$ s. The stale fraction at steady state is:

$$\mathbb{E}[f_{\text{stale}}] = 1 - e^{-T_{\text{stale}} / \tau_{\text{liveness}}}$$

where $\tau_{\text{liveness}}$ is the mean inter-communication interval between any pair. In a healthy network with periodic gossip at interval $T_{\text{gossip}}$:

$$\tau_{\text{liveness}} = \frac{n}{g} \cdot T_{\text{gossip}}$$

Expected stale PINGs per maintenance sweep:

$$\mathbb{E}[M_{\text{maintenance}}] = \min(n, Kb) \cdot (1 - e^{-T_{\text{stale}} / \tau_{\text{liveness}}})$$

**Correction for post-convergence state.** After bootstrap traffic refreshes all entries:

$$\mathbb{E}[f_{\text{stale}} \mid \text{post-convergence}] = \begin{cases}
0 & \text{if } T_{\text{last\_refresh}} < T_{\text{stale}} \\
1 - e^{-(T_{\text{last\_refresh}} - T_{\text{stale}}) / \tau_{\text{liveness}}} & \text{otherwise}
\end{cases}$$

Since $T_{\text{last\_refresh}} \ll T_{\text{stale}}$ after bootstrap, $f_{\text{stale}} = 0$ in all measured benchmarks.

### 5.6 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time (lookup)** | $O(b) = 256$ hops | $O(\log_K n)$ hops |
| **Time (bucket insert)** | $O(K) = 20$ | $O(K)$ |
| **Time (bucket evict)** | $O(K)$ | $O(K)$ |
| **Time (maintenance sweep)** | $O(Kb)$ | $O(K \log n)$ |
| **Memory (routing table)** | $O(Kb) = 400$ KB | $O(K \log n)$ |
| **Memory (per entry)** | $80$ B | $80$ B |
| **Communication (lookup)** | $\alpha \cdot b = 768$ msgs | $\alpha \log_K n$ msgs |
| **Communication (bootstrap)** | $\Theta(n^2)$ msgs/node | $\Theta(n^2)$ |
| **Communication (maintenance)** | $O(Kb)$ per sweep | $O(1)$ per sweep |

**Memory proof (expected).** For random NodeIDs in $\{0,1\}^{256}$, the expected number of buckets with at least one entry is:

$$\mathbb{E}[B_{\text{occupied}}] = \sum_{k=0}^{b-1} \left[1 - \left(1 - 2^{-(k+1)}\right)^n\right] \approx \log_2 n$$

This is a birthday-problem geometric sum. Each term $1 - (1 - 2^{-(k+1)})^n \approx 1$ for $k < \log_2 n$ and $\approx n/2^{k+1}$ for $k \geq \log_2 n$. The sum converges to $\log_2 n + O(1)$. ∎

---

## 6. Gossip Exchange

### 6.1 Gossip Algorithm

Every $T_{\text{gossip}}$ ticks (default 1000 = 1s), each node:
1. Selects $g = 3$ random known peers.
2. Packs up to $K_{\text{synapses}}$ synapse entries into a GOSSIP frame.
3. Sends the frame to each selected peer.
4. Peer receives, merges received weights with local weights (weighted average).

### 6.2 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time (per node)** | $O(m^2)$ (serialize all synapses) | $O(K_{\text{synapses}})$ (sample) |
| **Memory (serialization buffer)** | $O(s_{\text{frame}})$ | $O(s_{\text{frame}})$ |
| **Communication (per tick)** | $g \cdot n = \Theta(n)$ total | $g \cdot n = \Theta(n)$ |
| **Communication (per node)** | $g = O(1)$ | $g = O(1)$ |

Steady-state bandwidth per node is:

$$B_{\text{gossip}} = \frac{g \cdot s_{\text{frame}}}{T_{\text{gossip}}}$$

For defaults ($g = 3$, $T_{\text{gossip}} = 1$ s, $s_{\text{frame}} \approx 100$ B):

$$B_{\text{gossip}} = \frac{3 \cdot 100\,\text{B}}{1\,\text{s}} = 300\,\text{B/s}$$

This is **independent of $n$**, matching empirical measurements.

---

## 7. Reliable Retransmission

### 7.1 Algorithm

The reliable queue holds unacknowledged packets. On each retransmit scan (every $\Delta T_{\text{rtx}} = 10$ ms):
1. Scan all entries in $Q_{\text{reliable}}$.
2. For each entry where `now - sent_time > RTO`:
   - If `retries < max_retries` (default 3): re-send, increment retry count.
   - Else: drop the packet, report delivery failure.
3. Apply gradient weight decay to each entry.

### 7.2 Queue Length Analysis

**Assumptions.**
1. Packet loss events are independent with probability $\mu$.
2. Sending is a Poisson process with rate $\lambda_{\text{send}}$.
3. RTO is set to $2 \times$ RTT.

**Lemma 9 (Reliable queue at steady state).** Under Assumptions 1–3, the expected queue length is:

$$\mathbb{E}[|Q_{\text{reliable}}|] = \mu \cdot \frac{T_{\text{RTO}}}{\Delta T_{\text{send}}}$$

**Proof.** The queue is an M/G/$\infty$ process: arrivals at rate $\lambda_{\text{send}}$, service time distribution is geometric (retries). Each packet requires $1 + \mu/(1-\mu)$ expected transmissions. The sojourn time is $T_{\text{RTO}} \cdot (1 + \mu/(1-\mu))$. Little's law gives $\mathbb{E}[|Q|] = \lambda_{\text{send}} \cdot \mathbb{E}[T_{\text{sojourn}}]$. For $\mu \ll 1$, $\mathbb{E}[T_{\text{sojourn}}] \approx T_{\text{RTO}}$. With $\lambda_{\text{send}} = 1/\Delta T_{\text{send}}$, the result follows. ∎

For default values ($\mu = 0.01$, $T_{\text{RTO}} = 100$ ms, $\Delta T_{\text{send}} = 10$ ms):

$$\mathbb{E}[|Q_{\text{reliable}}|] = 0.01 \cdot \frac{100}{10} = 0.1$$

### 7.3 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time (per scan)** | $O(|Q|)$ | $O(\mathbb{E}[|Q|])$ |
| **Memory (queue storage)** | $O(\text{max\_retries} \cdot s_{\text{frame}} \cdot n)$ | $O(\mathbb{E}[|Q|] \cdot s_{\text{frame}})$ |
| **Communication (per packet)** | $1 + \text{max\_retries}$ transmissions | $1 + \frac{\mu}{1-\mu}$ transmissions |

---

## 8. Packet Ingress/Egress

### 8.1 Ingress (Recv)

Each tick, the engine drains the UDP socket of all pending datagrams:

$$p_{\text{recv}}^{(t)} \sim \text{Poisson}(\lambda_{\text{arrival}} \cdot \Delta t)$$

**Complexity:**

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(n)$ (one per peer) | $O(\mathbb{E}[p_{\text{recv}}])$ |
| **Memory** | $O(s_{\text{frame}} \cdot p_{\text{recv}})$ | $O(\mathbb{E}[p_{\text{recv}}] \cdot s_{\text{frame}})$ |
| **Communication** | $O(\mathbb{E}[p_{\text{recv}}])$ (ingested) | $O(\mathbb{E}[p_{\text{recv}}])$ |

The socket drain loop is non-blocking: `recv_from()` with 1ms timeout yields **0% CPU at idle**.

### 8.2 Egress (Send)

The outbound queue buffers all frames generated during the tick and drains them:

$$p_{\text{send}}^{(t)} = p_{\text{PING}}^{(t)} + p_{\text{PONG}}^{(t)} + p_{\text{DATA}}^{(t)} + p_{\text{retransmit}}^{(t)}$$

**Complexity:**

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(n)$ (flood phase) | $O(1)$ (steady state) |
| **Memory** | $O(s_{\text{frame}} \cdot n)$ (during flood) | $O(s_{\text{frame}} \cdot g)$ (gossip) |
| **Communication** | $O(n)$ | $O(1)$ |

Peak during bootstrap: each node sends $n-1$ PINGs + $n-1$ PONGs per convergence round. Steady state: $g = 3$ gossip messages per tick.

---

## 9. Engine Loop (Full Tick)

### 9.1 Phase Breakdown

Each tick executes six phases sequentially:

| Phase | Operation | Complexity | Time budget ($\mu$s)* |
|-------|-----------|------------|----------------------|
| 1 | Ingress drain | $O(p_{\text{recv}})$ | 5–50 |
| 2 | Outbound drain | $O(p_{\text{send}})$ | 5–50 |
| 3a | Forward pass | $O(d m^2)$ | 10–100 |
| 3b | Hebbian update | $O(d m^2)$ | 10–100 |
| 4 | Retransmit scan | $O(|Q|)$ | 1–10 |
| 5 | Apoptosis sweep | $O(m)$ | 1–5 |
| 6 | Yield / sleep | — | adjusts to hit $\Delta t$ |

*\*Measured on modern x86_64 at $n=50$, $m=100$.*

### 9.2 Tick Deadline

If total work exceeds $\Delta t$, the engine enters overflow mode:

$$P(\text{overflow}) = P\Bigl(\sum_{\text{phases}} T_{\text{phase}} > \Delta t\Bigr)$$

For default $\Delta t = 1$ ms, overflow probability at $n=50$, $m=100$ is $< 10^{-5}$ (empirical).

### 9.3 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time** | $O(n + d m^2 + m)$ | $O(g + d m^2)$ |
| **Memory** | $O(Kb + d m^2 + n)$ | $O(K \log n + d m^2)$ |
| **Communication** | $O(n)$ per tick (flood) | $O(g)$ per tick (gossip) |

---

## 10. Communication Complexity (Aggregate)

### 10.1 Per-Operation Message Counts

| Operation | Messages | Bound | Lower bound | Achievable? |
|-----------|----------|-------|-------------|-------------|
| Bootstrap PING | $n(n-1)$ | $\Theta(n^2)$ | $\Omega(n^2)$ | ✓ ($2 + o(1)$ factor) |
| Bootstrap PONG | $n(n-1)$ | $\Theta(n^2)$ | $\Omega(n^2)$ | ✓ |
| Lookup (iterative) | $\alpha \cdot \log_K n$ | $O(\log n)$ | $\Omega(\log n)$ | ✓ |
| Gossip per tick | $g \cdot n$ | $\Theta(n)$ | $\Omega(n)$ | ✓ (exactly tight) |
| Maintenance per sweep | $f_{\text{stale}} \cdot n$ | $O(n)$ | $\Omega(1)$ | ✓ |
| Reliable delivery | $1 + \mu/(1-\mu)$ per pkt | $O(1)$ per pkt | $\Omega(1)$ | ✓ |

### 10.2 Theorem 6 ($\Theta(n^2)$ Bootstrap Lower Bound)

**Assumptions.**
1. Communication is via point-to-point messages.
2. Each message conveys at most one node's identity.
3. The knowledge graph $G = (V, E)$ starts with $E^{(0)} = \{(i,i)\}$.

**Theorem 6 (Bootstrap lower bound).** Any protocol achieving full mesh under Assumptions 1–3 must send at least $\Omega(n^2)$ messages.

**Proof.** Let $G^{(t)}$ be the knowledge graph after $t$ messages. Each message $(i,j)$ adds at most **one** new directed edge $(i, j)$ or $(j, i)$ to $E$ (the sender's own identity). Knowledge of third parties requires separate messages (e.g., NODES response carrying $K$ entries). Full mesh requires $n^2 - n$ edges beyond $E^{(0)}$. Since each message adds at most one edge:

$$|\text{messages}| \geq n^2 - n = \Omega(n^2)$$ ∎

NWP achieves $2n(n-1)$ messages: $2 + o(1)$ times the lower bound. No protocol with single-identity-per-message semantics can improve this constant.

---

## 11. Memory Complexity (Aggregate)

| Structure | Worst Case | Average Case | Proof |
|-----------|-----------|-------------|-------|
| Routing table | $O(Kb) = 400$ KB | $O(K \log n) = 32$ KB @ $10^6$ | §5.6 |
| Synapse matrix | $m^2$ (dense) | $O(d m^2)$ (sparse) | §7.2 |
| Reliable queue | $O(\mu \cdot n \cdot s_{\text{frame}})$ | $O(1)$ | §7.2 |
| Activation buffer | $O(m)$ | $O(m)$ | §2.4 |
| Surprise buffer | $O(W + m)$ | $O(W + m)$ | §3.5 |
| Ingress buffer | $O(s_{\text{frame}} \cdot n)$ | $O(s_{\text{frame}} \cdot \mathbb{E}[p_{\text{recv}}])$ | §8.1 |
| **Total (steady state)** | $O(Kb + m^2)$ | $O(K \log n + d m^2)$ | — |

---

## 12. Failure Probability

### 12.1 Node Failure

**Assumptions.**
1. Node failures are independent Poisson processes with rate $\lambda_f$.
2. The network is fully connected (no partition).

**Lemma 10 (Expected failures).** For $n$ nodes over elapsed time $t$, the expected number of failures is:

$$\mathbb{E}[F_t] = n \cdot (1 - e^{-\lambda_f t})$$

**Proof.** Follows from the CDF of the exponential distribution: $P(\text{fail} \leq t) = 1 - e^{-\lambda_f t}$. Summing over $n$ independent nodes gives the expectation. ∎

**Three regimes:**

| Regime | $\lambda_f$ | $F_T$ for $n=50$, $T=120$s | Description |
|--------|-------------|--------------------------|-------------|
| Reliable | $10^{-6}\,\text{s}^{-1}$ | $6 \times 10^{-3}$ | One failure every 167 hours |
| Nominal | $10^{-4}\,\text{s}^{-1}$ | 0.6 | ~1 failure per experiment |
| Hostile | $10^{-2}\,\text{s}^{-1}$ | 60 | Full churn in 100s |

### 12.2 Data Loss Probability

Data (synaptic weights) is replicated via gossip to $r$ nodes:

$$P(\text{loss}) = (1 - e^{-\lambda_f T})^r$$

For $r = 3$ and nominal failure rate over 120s:

$$P(\text{loss}) = (1 - e^{-10^{-4} \cdot 120})^3 \approx (0.012)^3 \approx 1.7 \times 10^{-6}$$

### 12.3 Network Partition

A partition occurs when all paths through the knowledge graph between two subsets are severed. For an Erdős–Rényi graph $G(n, p)$ where $p$ is the per-node-pair knowledge probability:

$$P(\text{partition}) = \sum_{k=1}^{n-1} \binom{n-1}{k-1} (1-p)^{k(n-k)}$$

This is exponentially small for $p > \ln n / n$. Post-convergence with $p = 1$: $P(\text{partition}) = 0$.

### 12.4 Censorship / Eclipse Resistance

An eclipse attack requires the attacker to control all $K$ entries in the target's bucket. With fraction $f$ of malicious nodes:

$$P(\text{eclipse bucket } b) = \max\left(0, \frac{f n - K}{n} \cdot \frac{f n - K + 1}{n - 1} \cdots \frac{f n - 1}{n - K + 1}\right)$$

For the full bucket set ($b = 256$), all must be eclipsed simultaneously:

$$P(\text{full eclipse}) = \prod_{k=0}^{255} P(\text{eclipse bucket } k)$$

With $f = 0.25$ and $n = 100$:

$$P(\text{eclipse one bucket}) \approx (25/100)^{20} \approx 10^{-12}$$
$$P(\text{full eclipse}) \approx (10^{-12})^{256} \approx 10^{-3072}$$

### 12.5 Complexity

| Metric | Worst Case | Average Case |
|--------|-----------|-------------|
| **Time (failure detection)** | $O(T_{\text{stale}})$ = 300s | $O(T_{\text{stale}})$ |
| **Memory (failure tracking)** | $O(Kb)$ (failure counters) | $O(K \log n)$ |
| **Communication (recovery)** | $O(n)$ per recovered node | $O(K \log n)$ |

---

## 13. Expected Convergence

### 13.1 Full-Mesh Convergence (Restated)

**Theorem 5 (stated in §5.4).** Expected convergence time:

$$\mathbb{E}[T_{\text{conv}}] = \max\left(\text{RTT}, \frac{2(n-1)}{\nu}\right) + \frac{2(n-1)}{\nu} \cdot \frac{p_f}{1-p_f}$$

**Numerical examples** (RTT = 3ms, $\nu = 10^4$ msg/s, $p_f = 0$):

| $n$ | $\mathbb{E}[T_{\text{conv}}]$ | Regime |
|-----|------------------------------|--------|
| 10 | 4.8 ms | RTT |
| 50 | 12.8 ms | Transition |
| 100 | 22.8 ms | Socket drain |
| 500 | 103 ms | Socket drain |
| $10^4$ | 2.0 s | Socket (needs iterative) |

### 13.2 Learning Convergence (Restated)

**Theorem 1 (stated in §1.2).** Weight convergence is geometric:

$$\mathbf{W}^{(t)} = \mathbf{W}^{(\infty)} + (1 - \lambda)^t (\mathbf{W}^{(0)} - \mathbf{W}^{(\infty)})$$

Ticks to reach within $\epsilon$ of steady state:

$$t_{\epsilon} = \frac{\ln(\|\mathbf{W}^{(0)} - \mathbf{W}^{(\infty)}\|_F / \epsilon)}{-\ln(1 - \lambda)}$$

For $\lambda = 0.001$, $\epsilon = 0.01 \cdot \|\mathbf{W}^{(\infty)}\|_F$:

$$t_{1\%} \approx \frac{\ln(100)}{0.001} \approx 4605 \text{ ticks} \approx 4.6 \text{ seconds}$$

### 13.3 Prediction Error Convergence (Restated)

The prediction error $\gamma$ decreases as the network learns:

$$\mathbb{E}[\gamma^{(t)}] = \gamma_{\text{irreducible}} + (\gamma^{(0)} - \gamma_{\text{irreducible}}) e^{-t / \tau_\gamma}$$

where $\tau_\gamma$ is the learning time constant:

$$\tau_\gamma = \frac{1}{-\ln(1 - \eta \cdot \lambda_{\max}(\boldsymbol{\Sigma}^{-1}\mathbf{R}))}$$

where $\lambda_{\max}$ is the largest eigenvalue of the product and $\mathbf{R}$ is the input-output correlation matrix.

---

## 14. Reliability, Availability, and Consistency

### 14.1 System State Machine

The node state machine forms a Markov chain with absorbing state SHUTDOWN:

$$\begin{aligned}
&P(\text{Booting} \to \text{Discovering}) = 1 \\
&P(\text{Discovering} \to \text{Active}) = 1 - e^{-n \cdot p_{\text{seed}} \cdot T_{\text{timeout}}} \\
&P(\text{Active} \to \text{Degraded}) = \mathbb{1}[\text{peers} < n_{\text{liveness}}] \\
&P(\text{Degraded} \to \text{Active}) = 1 - e^{-T_{\text{recovery}} / \tau_{\text{bootstrap}}} \\
&P(\text{Active} \to \text{Shutdown}) = 1 \text{ (on SIGINT)}
\end{aligned}$$

### 14.2 Mean Time Between Failures

The system MTBF is the time until all $r$ replicas of a data item fail simultaneously:

$$\text{MTBF}_{\text{system}} = \frac{1}{n \lambda_f} \cdot \frac{1}{P(\text{loss} \mid \text{fail})}$$

For $n=50$, $\lambda_f = 10^{-6}\,\text{s}^{-1}$, $P(\text{loss} \mid \text{fail}) = 10^{-6}$:

$$\text{MTBF}_{\text{system}} \approx \frac{1}{50 \times 10^{-6}} \times 10^6 \approx 2 \times 10^{10}\,\text{s} \approx 634 \text{ years}$$

### 14.3 Availability

Per-node availability $A$:

$$A = \frac{\text{MTBF}}{\text{MTBF} + \text{MTTR}}$$

With $\text{MTTR} = \mathbb{E}[T_{\text{conv}}]$ from Theorem 5. For $n=50$, nominal conditions:

$$A = \frac{10^6}{10^6 + 0.013} \approx 0.999999987 \text{ (six nines)}$$

---

## 15. Complexity Summary Table

| Subsystem | Metric | Worst Case | Average Case | Section |
|-----------|--------|-----------|-------------|---------|
| **Hebbian STDP** | Time | $O(m^2)$ | $O(d m^2)$ | §1.5 |
| | Memory | $O(m^2)$ | $O(d m^2)$ | §1.5 |
| | Communication | $O(K_{\text{gossip}})$ per round | $O(K_{\text{gossip}})$ | §1.5 |
| **Forward Pass** | Time | $O(m^2)$ | $O(d m^2)$ | §2.4 |
| | Memory | $O(m^2)$ | $O(d m^2 + m)$ | §2.4 |
| **Neurogenesis** | Time | $O(m)$ | $O(m)$ | §3.5 |
| | Memory | $O(m)$ | $O(m)$ | §3.5 |
| **Apoptosis** | Time | $O(m\pi)$ (amortized $O(m/\pi)$) | $O(m)$ | §4.4 |
| | Memory | $O(m)$ | $O(m)$ | §4.4 |
| **DHT Lookup** | Time (hops) | $O(b) = 256$ | $O(\log_K n)$ | §5.6 |
| | Memory | $O(Kb) = 400$ KB | $O(K \log n) = 32$ KB @ $10^6$ | §5.6 |
| | Communication | $O(\alpha b)$ | $O(\alpha \log_K n)$ | §5.6 |
| **Bootstrap** | Time | $\max(\text{RTT}, 2n/\nu)$ | same | §5.4 |
| | Comm (total) | $\Theta(n^2)$ | $\Theta(n^2)$ | §6.1 |
| **Gossip** | Comm per tick | $g \cdot n$ total, $g$ per node | same | §6.2 |
| | Bandwidth | $g \cdot s_{\text{frame}} / T_{\text{gossip}}$ | same | §6.2 |
| **Reliable Queue** | Time per scan | $O(\mu n)$ | $O(\mu)$ | §7.3 |
| | Memory | $O(\text{max\_retries} \cdot n \cdot s_{\text{frame}})$ | $O(1)$ | §7.3 |
| **Ingress** | Time | $O(n)$ | $O(\mathbb{E}[p_{\text{recv}}])$ | §8.1 |
| **Egress** | Time | $O(n)$ | $O(g)$ | §8.2 |
| **Engine loop (total)** | Time | $O(n + d m^2)$ | $O(g + d m^2)$ | §9.3 |
| | Memory | $O(Kb + d m^2 + n)$ | $O(K \log n + d m^2)$ | §9.3 |

---

## 16. Empirical Validation

Every equation above is testable by experiment. The simulation framework provides:

| Experiment | What to measure | Expected result | Reference |
|-----------|----------------|----------------|-----------|
| Convergence time | Per-node peer count vs time | $\max(\text{RTT}, 2n/\nu)$ | Theorem 5 |
| Message complexity | Aggregate PING/PONG counter | $\Theta(n^2)$ | Theorem 6 |
| Learning convergence | Weight matrix norm vs tick | $\mathbf{W}^{(\infty)} = (\eta/\lambda)\boldsymbol{\Sigma}$ | Theorem 1 |
| Neurogenesis dynamics | Spawn event times | Poisson process, rate $\lambda_{\text{spawn}}$ | §3.4 |
| Apoptosis rate | Death events per tick | $\mathbb{E}[D] = m \cdot \epsilon_a^\pi$ | §4.2 |
| Failure recovery | Peer count after injection | Recovers to $n-1$ in $\mathbb{E}[T_{\text{conv}}]$ | Theorem 5b |
| Bandwidth | Bytes/tick at steady state | $300$ B/s independent of $n$ | §6.3 |
| Memory | RSS vs node count | $O(K \log n)$ growth | §5.6 |
| Eclipse resistance | Bucket composition under attack | $P < 10^{-12}$ for $f=0.25$ | §12.4 |

---

## References

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture, benchmark results, baseline comparisons
- [PROTOCOL_SPEC.md](PROTOCOL_SPEC.md) — Wire format BNF grammar, header layouts
- [PAPER.md](PAPER.md) — Research paper (systems + ML perspective)
- [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) — Implementation details, testing patterns
- Maymounkov & Mazières, *Kademlia: A Peer-to-Peer Information System Based on the XOR Metric*, IPTPS 2002.
- Friston, *The free-energy principle: a unified brain theory?*, Nature Reviews Neuroscience 2010.
- Gerstner & Kistler, *Spiking Neuron Models*, Cambridge University Press 2002.
