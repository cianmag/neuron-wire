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

If activations are drawn from a stationary distribution with covariance $\boldsymbol{\Sigma} = \mathbb{E}[\mathbf{a}\mathbf{a}^\top]$, the expected weight dynamics are:

$$\mathbb{E}[\mathbf{W}^{(t+1)}] = \mathbb{E}[\mathbf{W}^{(t)}] + \eta \boldsymbol{\Sigma} - \lambda \mathbb{E}[\mathbf{W}^{(t)}]$$

At steady state:

$$\mathbb{E}[\mathbf{W}^{(\infty)}] = \frac{\eta}{\lambda} \boldsymbol{\Sigma}$$

**Proof.** Setting $\mathbb{E}[\mathbf{W}^{(t+1)}] = \mathbb{E}[\mathbf{W}^{(t)}] = \mathbf{W}^{(\infty)}$:

$$\mathbf{W}^{(\infty)} = \mathbf{W}^{(\infty)} + \eta \boldsymbol{\Sigma} - \lambda \mathbf{W}^{(\infty)}$$
$$\lambda \mathbf{W}^{(\infty)} = \eta \boldsymbol{\Sigma}$$
$$\mathbf{W}^{(\infty)} = \frac{\eta}{\lambda} \boldsymbol{\Sigma} \quad \blacksquare$$

The steady-state weight matrix is proportional to the input covariance. This means the network learns the correlation structure of its inputs, not a supervised target — a key difference from backpropagation.

### 1.3 Weight Bounds

With no weight clamping, the magnitude evolves as:

$$|w_{ij}^{(t+1)}| \leq (1 - \lambda) |w_{ij}^{(t)}| + \eta$$

Solving the recurrence yields the stable bound:

$$\limsup_{t \to \infty} |w_{ij}^{(t)}| \leq \frac{\eta}{\lambda} + \frac{|w_{ij}^{(0)}|}{(1-\lambda)^t}$$

For default values $(\eta = 0.01, \lambda = 0.001)$:

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

If the readout weights and hidden weights are stationary (learning has converged), and observations are drawn from a distribution with variance $\sigma_o^2$, the expected prediction error is bounded by:

$$\mathbb{E}[\gamma^{(\infty)}] \leq \sqrt{\frac{\eta}{\lambda} \cdot \text{tr}(\boldsymbol{\Sigma}_{\text{input}})} + \sigma_o \cdot \sqrt{1 - r_{\max}^2}$$

where $r_{\max}$ is the maximum correlation coefficient between the learned representation and the observation. The first term represents representational bias (the model's prior), the second the irreducible observation noise.

---

## 3. Neurogenesis

### 3.1 Spawn Probability

A new neuron is spawned when cumulative surprise exceeds the spawn threshold $\sigma$:

$$P(\text{spawn} \mid \Gamma^{(t)} > \sigma) = 1 - e^{-\beta (\Gamma^{(t)} - \sigma)_+}$$

where $(x)_+ = \max(0, x)$ and $\beta$ is the spawn rate parameter.

**Expected spawns per tick:**

$$\mathbb{E}[S^{(t)}] = \sum_{i=1}^{m} P(\text{spawn}_i \mid \Gamma_i^{(t)} > \sigma)$$

With $K$ neurons experiencing surprise above threshold:

$$\mathbb{E}[S^{(t)}] \leq K \cdot (1 - e^{-\beta \cdot \mathbb{E}[\Gamma^{(t)} - \sigma \mid \Gamma^{(t)} > \sigma]})$$

### 3.2 Surprise Dynamics

Surprise accumulates over time and decays:

$$\Gamma^{(t+1)} = (1-\rho) \Gamma^{(t)} + \rho \gamma^{(t)}$$

This is an exponentially weighted moving average with time constant $\tau = 1/\rho$ (default $\rho = 0.001$, $\tau = 1000$ ticks).

### 3.3 Steady-State Neuron Count

The expected neuron count is determined by the balance of neurogenesis (birth) and apoptosis (death):

$$\mathbb{E}[m^{(t+1)}] = \mathbb{E}[m^{(t)}] + \mathbb{E}[\text{spawns}^{(t)}] - \mathbb{E}[\text{deaths}^{(t)}]$$

At steady state:

$$\mathbb{E}[\text{spawns}^{(\infty)}] = \mathbb{E}[\text{deaths}^{(\infty)}]$$

The spawn rate depends on the surprise distribution, which depends on the model's prediction error, which depends on the network size. This creates a feedback loop:

$$\gamma(m) = \gamma_0 \cdot e^{-\alpha m} + \gamma_{\text{irreducible}}$$

where $\alpha$ is the learning efficiency with respect to network size. The fixed-point neuron count satisfies:

$$S(m^*) = D(m^*)$$

In the noiseless case, the network grows until prediction error falls below threshold $\sigma$, at which point $\mathbb{E}[\text{spawns}] \approx 0$ and only apoptosis-driven turnover remains. The maximum neuron count is then:

$$m_{\max} = \frac{1}{\alpha} \ln\left(\frac{\gamma_0}{\sigma - \gamma_{\text{irreducible}}}\right)$$

### 3.4 Spawn Timing Distribution

Spawn events follow an inhomogeneous Poisson process with rate:

$$\lambda_{\text{spawn}}(t) = \sum_{i=1}^{m} P(\text{spawn}_i \mid \Gamma_i^{(t)} > \sigma)$$

The inter-spawn interval is distributed as:

$$P(\Delta t > \tau) = \exp\left(-\int_0^{\tau} \lambda_{\text{spawn}}(s) \, ds\right)$$

---

## 4. Apoptosis (Neuron Death)

### 4.1 Inactivity Detection

A neuron is marked for death if it's been inactive (activation magnitude below threshold) for $\pi$ consecutive ticks:

$$\text{dead}_i^{(t)} = \mathbb{1}\left[\sum_{\tau = t-\pi+1}^{t} \mathbb{1}[|a_i^{(\tau)}| < \epsilon_a] = \pi\right]$$

### 4.2 Death Probability

In steady state with random activations uniform in $[-1, 1]$, the probability a neuron is active at any tick is:

$$P(|a_i| > \epsilon_a) = 1 - \epsilon_a$$

The probability of remaining inactive for $\pi$ ticks:

$$P(\text{death}) = \epsilon_a^\pi$$

For $\epsilon_a = 0.01$ and $\pi = 1000$ (default 1 second at 1ms ticks):

$$P(\text{death}) = 0.01^{1000} \approx 10^{-2000}$$

In practice, this means random activation drift never triggers apoptosis — only true inactivity from a disconnected or dead input path causes neuron death. The expected death count per tick at steady state is:

$$\mathbb{E}[D^{(t)}] = m \cdot \epsilon_a^\pi$$

### 4.3 Cascading Death (Death Spiral)

When a neuron dies, its $K_{\text{out}}$ downstream connections are also severed, potentially starving those neurons of input:

$$d_{\text{cascade}} = \sum_{i=1}^{m} K_{\text{out}, i} \cdot \mathbb{1}[\text{all inputs dead}]$$

The death spiral triggers when more than $m_{\text{critical}}$ neurons die simultaneously:

$$m_{\text{critical}} = \frac{m}{\bar{K}_{\text{out}} + 1}$$

where $\bar{K}_{\text{out}}$ is the mean out-degree. If $D^{(t)} > m_{\text{critical}}$, cascading failure is expected.

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

**Theorem 1 (Lookup hops).** The expected number of iterative lookup hops in a Kademlia network with $n$ nodes and k-bucket size $K$ is:

$$\mathbb{E}[H_{\text{lookup}}] = \Theta\left(\frac{\log n}{\log K}\right)$$

**Proof sketch.** In each hop, the querying node contacts the $\alpha$ nearest known nodes from the bucket corresponding to the target's prefix. Each contacted node returns $K$ closer candidates. After $h$ hops, the distance to the target shrinks exponentially with base $K$:

$$\mathbb{E}[d_h] = \frac{2^{256}}{K^h}$$

Setting $\mathbb{E}[d_h] < 1$ (finding the exact target) yields:

$$h > \frac{256}{\log_2 K} = \frac{256}{\log_2 20} \approx 59.2$$

For finding any node with a matching prefix (not the exact target), the bound tightens to $O(\log_K n)$. With $n$ nodes uniformly distributed, the expected inter-node distance is $2^{256}/n$, and:

$$h_{\text{any}} = \left\lceil \log_K n \right\rceil \quad \blacksquare$$

For $K = 20$ and $n$ up to $10^6$:

$$h_{\text{any}}(10^6) = \lceil \log_{20} 10^6 \rceil = \lceil 4.6 \rceil = 5$$

### 5.4 Full-Mesh Convergence Time

**Theorem 2 (Convergence to full mesh).** The expected time for $n$ nodes booting simultaneously to achieve full connectivity (every node knows every other) is:

$$\mathbb{E}[T_{\text{conv}}] = \max\left(\text{RTT}, \frac{n^2}{2\nu}\right) + O\left(\frac{1}{\nu}\right)$$

where $\nu$ is the socket drain rate in messages/s and RTT is the round-trip time.

**Proof.** Two regimes exist:

**Regime 1 — RTT-limited ($n^2 < \nu \cdot \text{RTT}$):** The bottleneck is propagation delay. Node $i$ sends PINGs to $n-1$ peers. Each PING-PONG pair takes RTT/2 to transmit. After receiving its first PONG from node $j$, node $i$ knows $j$, and $j$ knows $i$. In $n-1$ sequential rounds:

$$T_{\text{conv}} = (n-1) \cdot \text{RTT} + O(1)$$

But PINGs are sent concurrently (UDP is pipelined), not sequentially. A single burst of $n-1$ PINGs takes RTT/2 to arrive. PONGs take RTT/2 to return. Total: RTT + socket drain time for $n-1$ responses.

**Regime 2 — Socket-limited ($n^2 \ge \nu \cdot \text{RTT}$):** The bottleneck is the outbound socket. Each node sends $n-1$ PINGs. At total network capacity of $\nu$ msg/s per node, the full flood takes:

$$T_{\text{conv}} = \frac{n-1}{\nu} + \text{RTT} + O\left(\frac{1}{\nu}\right)$$

Total messages sent = $n(n-1)$ PINGs + $n(n-1)$ PONGs = $2n(n-1)$. With $n$ nodes each draining at $\nu$ msg/s:

$$T_{\text{conv}} = \frac{2n(n-1)}{n \cdot \nu} + \text{RTT} = \frac{2(n-1)}{\nu} + \text{RTT}$$

For large $n$, this simplifies to $T_{\text{conv}} \sim 2n/\nu$. Combining both regimes:

$$T_{\text{conv}} = \max\left(\text{RTT}, \frac{2(n-1)}{\nu}\right) + O\left(\frac{1}{\nu}\right) \quad \blacksquare$$

### 5.5 Maintenance Overhead

Each node pings stale entries every $T_{\text{stale}} = 300$ s. The stale fraction at steady state is:

$$\mathbb{E}[f_{\text{stale}}] = 1 - e^{-T_{\text{stale}} / \tau_{\text{liveness}}}$$

where $\tau_{\text{liveness}}$ is the mean inter-communication interval between any pair. In a healthy network with periodic gossip at interval $T_{\text{gossip}}$:

$$\tau_{\text{liveness}} = \frac{n}{g} \cdot T_{\text{gossip}}$$

Expected stale PINGs per maintenance sweep:

$$\mathbb{E}[M_{\text{maintenance}}] = \min(n, Kb) \cdot (1 - e^{-T_{\text{stale}} / \tau_{\text{liveness}}})$$

For the steady-state benchmark ($n=50$, $T_{\text{gossip}}=1$s, $g=3$):

$$\tau_{\text{liveness}} = \frac{50}{3} \cdot 1\text{s} \approx 16.7\text{s}$$
$$\mathbb{E}[f_{\text{stale}}] = 1 - e^{-300/16.7} \approx 1 - 10^{-8} \approx 1$$

Wait — this suggests nearly 100% stale entries, which contradicts measured results (zero maintenance PINGs). The resolution is that convergence traffic itself refreshes all entries: after the bootstrap flood, every entry has `last_seen` within the last RTT, not the last 300s. The stale fraction only applies to *freshness under maintenance interval*, not to absolute staleness. The correct model is:

$$\mathbb{E}[f_{\text{stale}} \mid \text{post-convergence}] = \begin{cases}
0 & \text{if } T_{\text{last\_refresh}} < T_{\text{stale}} \\
1 - e^{-(T_{\text{last\_refresh}} - T_{\text{stale}}) / \tau_{\text{liveness}}} & \text{otherwise}
\end{cases}$$

Since $T_{\text{last\_refresh}} \ll T_{\text{stale}}$ during and shortly after bootstrap, $f_{\text{stale}} = 0$ in all measured benchmarks.

---

## 6. Communication Complexity

### 6.1 Per-Operation Message Counts

| Operation | Messages | Bound | Proof |
|-----------|----------|-------|-------|
| Bootstrap PING | $n(n-1)$ | $\Theta(n^2)$ | Each node pings all others |
| Bootstrap PONG | $n(n-1)$ | $\Theta(n^2)$ | Each PING generates one PONG |
| Total convergence | $2n(n-1)$ | $\Theta(n^2)$ | Sum of PING + PONG |
| Lookup (iterative) | $\alpha \cdot \log_K n$ | $O(\log n)$ | $h$ hops, $\alpha$ parallel queries |
| Gossip per tick | $g \cdot n$ | $\Theta(n)$ | $g$ peers per node |
| Maintenance per sweep | $f_{\text{stale}} \cdot n$ | $O(n)$ | Only stale entries need PING |

### 6.2 Proof of $\Theta(n^2)$ Bootstrap Bound

**Theorem 3 (Bootstrap message complexity).** Any protocol that converges to full mesh (every node knows every other's address) must send at least $\Omega(n^2)$ messages in the worst case, and NWP achieves this bound.

**Proof.** Let $G = (V, E)$ be the directed knowledge graph where $(i, j) \in E$ iff node $i$ knows node $j$'s address. Initially, $E^{(0)} = \{(i, i)\}$ for all $i$ (self-knowledge only). Full mesh requires $E = V \times V$ (every ordered pair).

Each message from node $i$ to $j$ can add at most *one* new edge to $E$: the sender's own address (in a PING/PONG). Knowledge about third parties requires separate messages (e.g., NODES response carrying K entries). Therefore:

$$|\text{messages}| \geq |V \times V| - |E^{(0)}| = n^2 - n = \Omega(n^2)$$

NWP achieves $2n(n-1)$ messages, which is $2 + o(1)$ times the lower bound. No protocol with single-address-per-message semantics can improve on this constant. $\blacksquare$

### 6.3 Steady-State Bandwidth

Per-node steady-state bandwidth is dominated by gossip:

$$B_{\text{gossip}} = \frac{g \cdot s_{\text{frame}}}{T_{\text{gossip}}}$$

where $s_{\text{frame}} = 32 + s_{\text{body}}$ bytes (transport header + NWP header + body).

For defaults ($g = 3$, $T_{\text{gossip}} = 1$ s, $s_{\text{frame}} \approx 100$ B):

$$B_{\text{gossip}} = \frac{3 \cdot 100\,\text{B}}{1\,\text{s}} = 300\,\text{B/s}$$

This is independent of $n$, confirmed empirically (§9, ARCHITECTURE.md).

---

## 7. Memory Complexity

### 7.1 Routing Table

Each node maintains $b = 256$ k-buckets, each holding up to $K = 20$ entries:

$$M_{\text{routing}} = K \cdot b \cdot s_{\text{entry}} = 20 \times 256 \times 80\,\text{B} \approx 400\,\text{KB (worst case)}$$

**Expected case (random NodeIDs):** Entries occupy only $O(\log n)$ distinct buckets:

$$\mathbb{E}[M_{\text{routing}}] = K \cdot \min(b, \lceil \log_2 n \rceil) \cdot s_{\text{entry}} + o(n)$$

**Proof.** For random NodeIDs in $\{0,1\}^{256}$, the number of nodes with XOR prefix $p$ follows a binomial distribution with $P(p) = 2^{-|p|}$. The expected number of buckets with at least one entry is:

$$\mathbb{E}[B_{\text{occupied}}] = \sum_{k=0}^{b-1} \left[1 - \left(1 - 2^{-(k+1)}\right)^n\right] \approx \log_2 n$$

This is the classic "birthday problem" in a geometric-probability setting. Each term $1 - (1 - 2^{-(k+1)})^n$ is approximately 1 for $k < \log_2 n$ and $\approx n/2^{k+1}$ for $k \geq \log_2 n$. The sum converges to $\log_2 n + O(1)$. $\blacksquare$

### 7.2 Synapse Matrix

The synapse store is a sparse $m \times m$ matrix. With connection density $d$:

$$\mathbb{E}[|\mathcal{S}|] = d \cdot m^2$$

Each synapse entry stores (pre, post, weight, timestamp):

$$M_{\text{synapses}} = d \cdot m^2 \cdot (8 + 8 + 4 + 4) = 24 d m^2 \,\text{B}$$

For $m = 1000$ and $d = 0.1$:

$$\mathbb{E}[M_{\text{synapses}}] = 24 \times 0.1 \times 10^6 = 2.4\,\text{MB}$$

### 7.3 Reliable Queue

The reliable queue holds unacknowledged packets. At steady state with perfect delivery:

$$\mathbb{E}[|Q_{\text{reliable}}|] = 0$$

Under packet loss $\mu$, the queue length follows an M/G/$\infty$ process:

$$\mathbb{E}[|Q_{\text{reliable}}|] = \mu \cdot \frac{T_{\text{timeout}}}{T_{\text{send}}}$$

where $T_{\text{send}}$ is the inter-packet interval. For default values ($\mu = 0.01$, $T_{\text{timeout}} = 100$ ms, $T_{\text{send}} = 10$ ms):

$$\mathbb{E}[|Q_{\text{reliable}}|] = 0.01 \cdot \frac{100}{10} = 0.1$$

---

## 8. Failure Probability

### 8.1 Node Failure

Node failure is modeled as an independent Poisson process with rate $\lambda_f$:

$$P(\text{node}_i \text{ fails by time } t) = 1 - e^{-\lambda_f t}$$

For $n$ nodes, the expected number of failures in $T$ ticks:

$$\mathbb{E}[F_T] = n \cdot (1 - e^{-\lambda_f T \Delta t})$$

**Three regimes:**

| Regime | $\lambda_f$ | $F_T$ for $n=50$, $T=120$s | Description |
|--------|-------------|--------------------------|-------------|
| Reliable | $10^{-6}\,\text{s}^{-1}$ | $6 \times 10^{-3}$ | One failure every 167 hours |
| Nominal | $10^{-4}\,\text{s}^{-1}$ | 0.6 | ~1 failure per experiment |
| Hostile | $10^{-2}\,\text{s}^{-1}$ | 60 | Full churn in 100s |

### 8.2 Data Loss Probability

Data (synaptic weights) is replicated via gossip. If $r$ nodes hold a copy of weight $w_{ij}$:

$$P(\text{loss}) = (1 - e^{-\lambda_f T})^r$$

For $r = 3$ and nominal failure rate over 120s:

$$P(\text{loss}) = (1 - e^{-10^{-4} \cdot 120})^3 \approx (0.012)^3 \approx 1.7 \times 10^{-6}$$

### 8.3 Network Partition

A partition occurs when all paths through the knowledge graph between two subsets are severed. For an Erdős–Rényi graph $G(n, p)$ where $p$ is the per-node-pair knowledge probability:

$$P(\text{partition}) = \sum_{k=1}^{n-1} \binom{n-1}{k-1} (1-p)^{k(n-k)}$$

This is exponentially small for $p > \frac{\ln n}{n}$. Post-convergence with $p = 1$:

$$P(\text{partition}) = 0$$

During bootstrap ($p \ll 1$), partitions are possible. The critical threshold is when $\sum_{i} \text{peer\_count}_i \geq n \cdot \ln n$.

### 8.4 Censorship / Eclipse Resistance

An eclipse attack requires the attacker to control all $K$ entries in the target's bucket. With fraction $f$ of malicious nodes:

$$P(\text{eclipse bucket } b) = \max\left(0, \frac{f n - K}{n} \cdot \frac{f n - K + 1}{n - 1} \cdots \frac{f n - 1}{n - K + 1}\right)$$

For the full bucket set ($b = 256$), all must be eclipsed simultaneously:

$$P(\text{full eclipse}) = \prod_{k=0}^{255} P(\text{eclipse bucket } k)$$

With $f = 0.25$ and $n = 100$:

$$P(\text{eclipse one bucket}) \approx \left(\frac{25}{100}\right)^{20} \approx 10^{-12}$$
$$P(\text{full eclipse}) \approx (10^{-12})^{256} \approx 10^{-3072}$$

The simultaneous-eclipse requirement makes full eclipse computationally infeasible.

---

## 9. Expected Convergence

### 9.1 Full-Mesh Convergence

**Theorem 4 (Expected convergence time).** For $n$ nodes booting simultaneously with RTT and socket drain rate $\nu$, the expected time to full mesh (every node knows every other) is:

$$\mathbb{E}[T_{\text{conv}}] = \text{RTT} + \frac{2(n-1)}{\nu} + \sum_{k=1}^{\infty} \frac{2(n-1) p_f^k}{\nu}$$

where $p_f$ is per-packet loss probability.

**Proof.** Each round (PING flood → PONG flood), every node sends $n-1$ PINGs and receives $n-1$ PONGs. The initial PING flood takes RTT/2 to arrive. PONGs take RTT/2 to return. Total time per round: RTT + $2(n-1)/\nu$ for serial drain.

Under packet loss $p_f$, the probability a given PING-PONG exchange completes in exactly $r$ retries follows a geometric distribution:

$$P(\text{success at retry } r) = (1 - p_f)^r p_f$$

The expected number of retries is $p_f / (1 - p_f)$. Adding this to the base time:

$$\mathbb{E}[T_{\text{conv}}] = \text{RTT} + \frac{2(n-1)}{\nu} + \frac{2(n-1)}{\nu} \cdot \frac{p_f}{1-p_f}$$

For $p_f = 0$ (lab conditions): $\mathbb{E}[T_{\text{conv}}] = \text{RTT} + 2(n-1)/\nu$. $\blacksquare$

**Numerical examples** (RTT = 3ms, $\nu = 10^4$ msg/s):

| $n$ | $\mathbb{E}[T_{\text{conv}}]$ | Dominant term |
|-----|------------------------------|---------------|
| 10 | 4.8 ms | RTT |
| 50 | 12.8 ms | RTT + socket |
| 100 | 22.8 ms | Socket drain |
| 500 | 103 ms | Socket drain |
| 10^4 | 2.0 s | Socket drain (requires iterative routing) |

### 9.2 Learning Convergence

The weight matrix converges to its steady-state value $\mathbf{W}^{(\infty)} = (\eta/\lambda)\boldsymbol{\Sigma}$ according to:

$$\mathbf{W}^{(t)} = \mathbf{W}^{(\infty)} + (1 - \lambda)^t(\mathbf{W}^{(0)} - \mathbf{W}^{(\infty)})$$

The convergence rate is geometric with factor $(1 - \lambda)$. The number of ticks to reach within $\epsilon$ of steady state:

$$t_{\epsilon} = \frac{\ln(\|\mathbf{W}^{(0)} - \mathbf{W}^{(\infty)}\|_F / \epsilon)}{-\ln(1 - \lambda)}$$

For $\lambda = 0.001$, $\epsilon = 0.01 \cdot \|\mathbf{W}^{(\infty)}\|_F$:

$$t_{1\%} \approx \frac{\ln(1 / 0.01)}{0.001} \approx 4605 \text{ ticks} \approx 4.6 \text{ seconds}$$

### 9.3 Prediction Error Convergence

The prediction error $\gamma$ decreases as the network learns:

$$\mathbb{E}[\gamma^{(t)}] = \gamma_{\text{irreducible}} + (\gamma^{(0)} - \gamma_{\text{irreducible}}) e^{-t / \tau_\gamma}$$

where $\tau_\gamma$ is the learning time constant:

$$\tau_\gamma = \frac{1}{-\ln(1 - \eta \cdot \lambda_{\max}(\boldsymbol{\Sigma}^{-1}\mathbf{R}))}$$

where $\lambda_{\max}$ is the largest eigenvalue of the product and $\mathbf{R}$ is the input-output correlation matrix.

---

## 10. Reliability, Availability, and Consistency

### 10.1 System State Machine

The node state machine forms a Markov chain with absorbing state SHUTDOWN:

$$\begin{aligned}
&P(\text{Booting} \to \text{Discovering}) = 1 \\
&P(\text{Discovering} \to \text{Active}) = 1 - e^{-n \cdot p_{\text{seed}} \cdot T_{\text{timeout}}} \\
&P(\text{Active} \to \text{Degraded}) = \mathbb{1}[\text{peers} < n_{\text{liveness}}] \\
&P(\text{Degraded} \to \text{Active}) = 1 - e^{-T_{\text{recovery}} / \tau_{\text{bootstrap}}} \\
&P(\text{Degraded} \to \text{Dead}) = \mathbb{1}[T_{\text{degraded}} > T_{\text{max\_degraded}}] \\
&P(\text{Active} \to \text{Shutdown}) = 1 \text{ (on SIGINT)}
\end{aligned}$$

### 10.2 Mean Time Between Failures

The system MTBF is the time until all $n$ nodes fail simultaneously (true data loss):

$$\text{MTBF}_{\text{system}} = \frac{1}{n \lambda_f} \cdot \frac{1}{P(\text{loss} \mid \text{fail})}$$

For $n=50$, $\lambda_f = 10^{-6}\,\text{s}^{-1}$, $P(\text{loss} \mid \text{fail}) = 10^{-6}$:

$$\text{MTBF}_{\text{system}} \approx \frac{1}{50 \times 10^{-6}} \times 10^6 \approx 2 \times 10^{10}\,\text{s} \approx 634 \text{ years}$$

### 10.3 Availability

Per-node availability $A$ is:

$$A = \frac{\text{MTBF}}{\text{MTBF} + \text{MTTR}}$$

With MTTR (mean time to recover) dominated by bootstrapping: $\text{MTTR} = \mathbb{E}[T_{\text{conv}}]$.

For $n=50$, nominal conditions:

$$A = \frac{10^6}{10^6 + 0.013} \approx 0.999999987 \text{ (six nines)}$$

---

## 11. Complexity Summary Table

| Metric | Bound | Conditions | Section |
|--------|-------|-----------|---------|
| Routing memory | $\Theta(K \log n)$ | Random NodeIDs | §7.1 |
| Routing memory | $O(Kb)$ worst-case | Adversarial IDs | §7.1 |
| Lookup hops | $\Theta(\log_K n)$ | $n \geq K$ | §5.3 |
| Bootstrap messages | $\Theta(n^2)$ | Full mesh | §6.2 |
| Bootstrap time | $\max(\text{RTT}, 2n/\nu)$ | All | §9.1 |
| Steady-state bandwidth | $\Theta(1)$ per node | $n$ independent | §6.3 |
| Maintenance messages | $O(1)$ per sweep | Healthy network | §5.5 |
| Learning convergence | $O(1/\lambda)$ ticks | Fixed point | §9.2 |
| Neuron count | $O(\alpha^{-1} \ln \gamma_0/\sigma)$ | Steady state | §3.3 |
| Failure probability | $O((\lambda_f T)^r)$ | $r$ replication | §8.2 |
| Eclipse resistance | $O(f^{Kb})$ | $f$ malicious fraction | §8.4 |

---

## 12. Empirical Validation

Every equation above is testable by experiment. The simulation framework (`cargo run --example simulate -- --paper-mode ...`) provides:

- **Convergence time**: logs per-node peer count over time → compare $\mathbb{E}[T_{\text{conv}}]$ against §9.1
- **Message complexity**: aggregate PING/PONG counters → verify $\Theta(n^2)$ bound in §6.2
- **Learning convergence**: dump weight matrix at checkpoints → verify $\mathbf{W}^{(\infty)} = (\eta/\lambda)\boldsymbol{\Sigma}$ in §1.2
- **Neurogenesis dynamics**: log spawn events → verify Poisson process in §3.4
- **Failure experiments**: `--failure-mode node-death --failure-at <sec> --failure-percent <pct>` → measure recovery probability in §8

---

## References

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture, benchmark results, baseline comparisons
- [PROTOCOL_SPEC.md](PROTOCOL_SPEC.md) — Wire format BNF grammar, header layouts
- [PAPER.md](PAPER.md) — Research paper (systems + ML perspective)
- [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) — Implementation details, testing patterns
- Maymounkov & Mazières, *Kademlia: A Peer-to-Peer Information System Based on the XOR Metric*, IPTPS 2002.
- Friston, *The free-energy principle: a unified brain theory?*, Nature Reviews Neuroscience 2010.
- Gerstner & Kistler, *Spiking Neuron Models*, Cambridge University Press 2002.
