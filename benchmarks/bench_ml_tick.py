#!/usr/bin/env python3
"""Performance benchmark for MLSystem::tick() simulation.

Usage:
    python benchmarks/bench_ml_tick.py --neurons 1000 --synapses-per 100
    python benchmarks/bench_ml_tick.py --sweep
"""
import time
import math
import random
import argparse
from typing import Dict, Tuple


class AdaptiveLROptimiser:
    """Per-synapse adaptive LR (Adam)."""

    def __init__(self, eta: float = 0.001):
        self.eta = eta
        self.state: Dict[Tuple[int, int], Dict] = {}

    def update(self, sid: Tuple[int, int], grad: float) -> float:
        s = self.state.setdefault(sid, {"g2": 0.0, "m": 0.0, "t": 0})
        s["t"] += 1
        s["g2"] += grad * grad
        beta1, beta2 = 0.9, 0.999
        s["m"] = beta1 * s["m"] + (1 - beta1) * grad
        m_hat = s["m"] / (1 - beta1 ** s["t"])
        v_hat = s["g2"] / s["t"] / (1 - beta2 ** s["t"])
        return self.eta * m_hat / (math.sqrt(v_hat) + 1e-8)


class MLSystemSim:
    """Simulate MLSystem::tick() with N neurons × M synapses."""

    def __init__(self, n_neurons: int, synapses_per: int):
        self.gradients: Dict[Tuple[int, int], float] = {}
        self.weights: Dict[Tuple[int, int], float] = {}
        for pre in range(n_neurons):
            posts = random.sample(range(n_neurons), min(synapses_per, n_neurons))
            for post in posts:
                if pre != post:
                    self.gradients[(pre, post)] = random.uniform(-0.5, 0.5)
                    self.weights[(pre, post)] = random.uniform(-1.0, 1.0)
        self.optimiser = AdaptiveLROptimiser()

    def tick(self):
        for sid, grad in self.gradients.items():
            eta_eff = self.optimiser.update(sid, grad)
            w = self.weights[sid]
            delta = eta_eff * grad
            self.weights[sid] = max(-5.0, min(5.0, w + delta))


def benchmark(n_neurons: int, syn_per: int, n_ticks: int, warmup: int = 50) -> Dict:
    ml = MLSystemSim(n_neurons, syn_per)
    for _ in range(warmup):
        ml.tick()

    start = time.perf_counter()
    for _ in range(n_ticks):
        ml.tick()
    elapsed = time.perf_counter() - start

    total = len(ml.gradients)
    tps = n_ticks / elapsed
    return {
        "neurons": n_neurons,
        "syn_per": syn_per,
        "total_synapses": total,
        "ticks": n_ticks,
        "elapsed_s": round(elapsed, 3),
        "ticks_per_sec": round(tps, 1),
        "ms_per_tick": round(1000 / tps, 4),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--neurons", type=int, default=500)
    parser.add_argument("--synapses-per", type=int, default=50)
    parser.add_argument("--ticks", type=int, default=500)
    parser.add_argument("--sweep", action="store_true")
    args = parser.parse_args()

    print("=" * 65)
    print("MLSystem::tick() Perf Benchmark (Python sim)")
    print("=" * 65)

    if args.sweep:
        configs = [
            (100, 20),
            (200, 30),
            (500, 20),
            (1000, 10),
        ]
        print(f"\n{'Neurons':>7} {'S/N':>5} {'Synapses':>9} {'Ticks/s':>10} {'ms/tick':>9}")
        print("-" * 45)
        for n, s in configs:
            r = benchmark(n, s, n_ticks=200, warmup=30)
            print(f"{r['neurons']:>7} {r['syn_per']:>5} {r['total_synapses']:>9,} "
                  f"{r['ticks_per_sec']:>10.1f} {r['ms_per_tick']:>9.4f}")
    else:
        r = benchmark(args.neurons, args.synapses_per, args.ticks)
        print(f"\n  Neurons:        {r['neurons']:>6,}")
        print(f"  Syn/neuron:     {r['syn_per']:>6}")
        print(f"  Total synapses: {r['total_synapses']:>6,}")
        print(f"  Ticks:          {r['ticks']:>6}")
        print(f"  Elapsed:        {r['elapsed_s']:>6.3f}s")
        print(f"  Throughput:     {r['ticks_per_sec']:>8,.1f} ticks/s")
        print(f"  Per tick:       {r['ms_per_tick']:>8.4f}ms")
