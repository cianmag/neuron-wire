#!/usr/bin/env python3
"""Dogfood run — NWP engine simulation with MLSystem active.

Sets up a small neural network, runs ticks with ML integration,
and logs behavior to CSV for analysis.

Usage:
    python benchmarks/dogfood_ml.py --ticks 100 --neurons 50
    python benchmarks/dogfood_ml.py --ticks 500 --neurons 200 --output dogfood_log.csv
"""
import time
import math
import random
import argparse
from typing import Dict, Tuple, List


class MLSystemDogfood:
    """End-to-end MLSystem simulation (adaptive LR + meta + curiosity + memory)."""

    def __init__(self, n_neurons: int, syn_per: int):
        self.n_neurons = n_neurons
        self.tick_num = 0
        self.total_synapses = 0

        # Activations
        self.activations: Dict[int, float] = {}

        # Synapses: weights[pre][post] = w
        self.weights: Dict[Tuple[int, int], float] = {}
        self.gradients: Dict[Tuple[int, int], float] = {}

        # Build random graph
        for pre in range(n_neurons):
            self.activations[pre] = random.uniform(-0.5, 0.5)
            posts = random.sample(range(n_neurons), min(syn_per, n_neurons))
            for post in posts:
                if pre != post:
                    self.weights[(pre, post)] = random.uniform(-1.0, 1.0)
                    self.gradients[(pre, post)] = 0.0

        self.total_synapses = len(self.weights)

        # ML state
        self.optim_state: Dict[Tuple[int, int], Dict] = {}
        self.visit_counts: Dict[int, int] = {}
        self.memory: List[Dict] = []
        self.memory_capacity = 100
        self.eta = 0.01
        self.gamma_ewc = 100.0

        # Stats
        self.surprise_log: List[float] = []
        self.curiosity_log: List[float] = []
        self.mean_weight_log: List[float] = []
        self.gradient_norm_log: List[float] = []

    def tick(self) -> Dict:
        """Run one full tick: forward → gradients → ML update → log."""
        self.tick_num += 1

        # 1. Forward pass (simple propagation)
        new_activations = {}
        for pre in range(self.n_neurons):
            a_pre = self.activations[pre]
            for post in range(self.n_neurons):
                key = (pre, post)
                if key in self.weights:
                    contribution = a_pre * self.weights[key]
                    new_activations[post] = new_activations.get(post, 0.0) + contribution

        # Apply activation function (tanh)
        for nid in new_activations:
            new_activations[nid] = math.tanh(new_activations[nid])

        # 2. Compute prediction error / surprise
        prediction_errors = []
        for nid in range(self.n_neurons):
            predicted = new_activations.get(nid, 0.0)
            actual = self.activations.get(nid, 0.0)
            error = abs(predicted - actual)
            prediction_errors.append(error)

        avg_error = sum(prediction_errors) / len(prediction_errors) if prediction_errors else 0.0

        # 3. Hebbian-like gradients
        total_grad_norm = 0.0
        for pre in range(self.n_neurons):
            a_pre = self.activations[pre]
            for post in range(self.n_neurons):
                key = (pre, post)
                if key in self.weights:
                    a_post = new_activations.get(post, 0.0)
                    # Hebbian: Δw = η * a_pre * a_post
                    grad = a_pre * a_post
                    self.gradients[key] = grad
                    total_grad_norm += abs(grad)

        # 4. ML update: adaptive LR
        for key, grad in self.gradients.items():
            if key not in self.optim_state:
                self.optim_state[key] = {"g2": 0.0, "m": 0.0, "t": 0}
            s = self.optim_state[key]
            s["t"] += 1
            s["g2"] += grad * grad
            beta1, beta2 = 0.9, 0.999
            s["m"] = beta1 * s["m"] + (1 - beta1) * grad
            m_hat = s["m"] / (1 - beta1 ** s["t"])
            v_hat = s["g2"] / s["t"] / (1 - beta2 ** s["t"])
            eta_eff = self.eta * m_hat / (math.sqrt(v_hat) + 1e-8)

            delta = eta_eff * grad

            # EWC correction
            w = self.weights[key]
            ewc_penalty = -self.gamma_ewc * 0.01 * (w - 0.0)
            self.weights[key] = max(-5.0, min(5.0, w + delta + 1e-6 * ewc_penalty))

        # 5. Curiosity
        obs_hash = hash(frozenset(self.activations.items()))
        self.visit_counts[obs_hash] = self.visit_counts.get(obs_hash, 0) + 1
        count_bonus = 1.0 / math.sqrt(self.visit_counts[obs_hash] + 1)
        curiosity = 0.1 * count_bonus + 0.05 * avg_error

        # 6. Store observation in memory (LRU)
        self.memory.append({
            "tick": self.tick_num,
            "avg_error": avg_error,
            "curiosity": curiosity,
            "n_synapses": self.total_synapses,
            "grad_norm": total_grad_norm,
        })
        if len(self.memory) > self.memory_capacity:
            self.memory.pop(0)

        # 7. Update activations
        self.activations = new_activations

        # Log
        self.surprise_log.append(avg_error)
        self.curiosity_log.append(curiosity)
        self.mean_weight_log.append(
            sum(abs(w) for w in self.weights.values()) / len(self.weights)
        )
        self.gradient_norm_log.append(total_grad_norm)

        return {
            "tick": self.tick_num,
            "avg_surprise": avg_error,
            "curiosity": curiosity,
            "mean_weight": self.mean_weight_log[-1],
            "gradient_norm": total_grad_norm,
            "active_synapses": self.total_synapses,
        }

    def report(self) -> str:
        """Generate statistical summary of the run."""
        if not self.surprise_log:
            return "No data"

        n = len(self.surprise_log)
        return (
            f"MLSystem Dogfood Report ({n} ticks)\n"
            f"{'-' * 40}\n"
            f"Surprise    — mean={sum(self.surprise_log)/n:.4f}  "
            f"min={min(self.surprise_log):.4f}  max={max(self.surprise_log):.4f}\n"
            f"Curiosity   — mean={sum(self.curiosity_log)/n:.4f}  "
            f"min={min(self.curiosity_log):.4f}  max={max(self.curiosity_log):.4f}\n"
            f"Weight mag  — mean={sum(self.mean_weight_log)/n:.4f}  "
            f"start={self.mean_weight_log[0]:.4f}  end={self.mean_weight_log[-1]:.4f}\n"
            f"Grad norm   — mean={sum(self.gradient_norm_log)/n:.4f}  "
            f"start={self.gradient_norm_log[0]:.4f}  end={self.gradient_norm_log[-1]:.4f}\n"
            f"Total synapses: {self.total_synapses}  "
            f"Memory used: {len(self.memory)}/{self.memory_capacity}"
        )

    def export_csv(self) -> str:
        """Return CSV of all tick data."""
        lines = ["tick,surprise,curiosity,mean_weight,gradient_norm"]
        for i in range(len(self.surprise_log)):
            lines.append(
                f"{i+1},{self.surprise_log[i]:.6f},{self.curiosity_log[i]:.6f},"
                f"{self.mean_weight_log[i]:.6f},{self.gradient_norm_log[i]:.6f}"
            )
        return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Dogfood MLSystem run")
    parser.add_argument("--ticks", type=int, default=200, help="Ticks to simulate")
    parser.add_argument("--neurons", type=int, default=100, help="Number of neurons")
    parser.add_argument("--synapses", type=int, default=20,
                        help="Synapses per neuron")
    parser.add_argument("--output", default=None,
                        help="Save CSV log to file")
    parser.add_argument("--quiet", action="store_true",
                        help="Only print final report")
    args = parser.parse_args()

    if not args.quiet:
        print(f"Dogfood: {args.neurons}n × {args.synapses}s = "
              f"{args.neurons * args.synapses} synapses, {args.ticks} ticks")

    ml = MLSystemDogfood(args.neurons, args.synapses)
    start = time.perf_counter()

    for _ in range(args.ticks):
        report = ml.tick()
        if not args.quiet and report["tick"] % 50 == 0:
            print(f"  tick={report['tick']:>4}  surprise={report['avg_surprise']:.4f}  "
                  f"curiosity={report['curiosity']:.4f}  "
                  f"|w|={report['mean_weight']:.4f}")

    elapsed = time.perf_counter() - start
    ticks_per_sec = args.ticks / elapsed if elapsed > 0 else 0

    print(f"\n{ml.report()}")
    print(f"\nRuntime: {elapsed:.2f}s ({ticks_per_sec:.0f} ticks/s)")

    if args.output:
        csv = ml.export_csv()
        with open(args.output, "w") as f:
            f.write(csv)
        print(f"CSV saved to {args.output}")
