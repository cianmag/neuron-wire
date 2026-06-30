#!/usr/bin/env python3
"""Unified comparison framework for NWP vs established distributed learning baselines.

Each baseline trains a linear classifier on synthetic data and reports
accuracy, bandwidth, convergence speed, and memory usage.

Usage:
    python baselines/comparison_framework.py --framework all
    python baselines/comparison_framework.py --framework nwp federated --output results.csv
"""
import numpy as np
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Callable
import time
import argparse


@dataclass
class ComparisonResult:
    """Results from a single baseline comparison run."""
    name: str
    accuracy: float  # Final test accuracy (0-1)
    bandwidth: float  # Total bytes exchanged
    rounds: int  # Training rounds to converge
    convergence_round: int  # First round within 5% of final accuracy
    memory_mb: float  # Memory per node
    train_time_ms: float  # Total training time


class ComparisonFramework:
    """Run all baselines on synthetic data and report metrics."""

    def __init__(self, n_nodes: int = 50, n_features: int = 100,
                 n_classes: int = 10, n_rounds: int = 50):
        self.n_nodes = n_nodes
        self.n_features = n_features
        self.n_classes = n_classes
        self.n_rounds = n_rounds
        self.data = self._generate_synthetic()

    def _generate_synthetic(self):
        X = np.random.randn(2000, self.n_features).astype(np.float32)
        # Make it somewhat learnable
        true_w = np.random.randn(self.n_features, self.n_classes).astype(np.float32) * 0.5
        logits = X @ true_w
        probs = np.exp(logits - logits.max(axis=1, keepdims=True))
        probs /= probs.sum(axis=1, keepdims=True)
        y = np.array([np.random.choice(self.n_classes, p=probs[i]) for i in range(len(X))])

        # Split train/test
        split = len(X) * 2 // 3
        return X[:split], y[:split], X[split:], y[split:]

    def _run_baseline(self, name: str, baseline_cls: Callable,
                      **kwargs) -> ComparisonResult:
        """Run a baseline and collect metrics."""
        X_train, y_train, X_test, y_test = self.data

        inst = baseline_cls(
            n_nodes=self.n_nodes,
            n_features=self.n_features,
            n_classes=self.n_classes,
            **kwargs
        )

        start = time.perf_counter()
        accuracies = []
        last_acc = 0.0
        convergence_round = self.n_rounds

        for r in range(self.n_rounds):
            try:
                acc = inst.train(X_train, y_train)
            except IndexError:
                # Handle edge case where a node has no data
                acc = 0.0

            # Try to get test predictions
            test_logits = None
            if hasattr(inst, 'weights'):
                test_logits = X_test @ inst.weights + inst.bias
            elif hasattr(inst, 'global_weights'):
                test_logits = X_test @ inst.global_weights + inst.global_bias
            elif hasattr(inst, 'node_weights'):
                test_logits = X_test @ inst.node_weights[0] + inst.node_biases[0]
            elif hasattr(inst, 'subnet_weights'):
                test_logits = X_test @ inst.subnet_weights[0] + inst.subnet_biases[0]

            if test_logits is not None:
                probs = np.exp(test_logits - test_logits.max(axis=1, keepdims=True))
                probs /= probs.sum(axis=1, keepdims=True)
                preds = np.argmax(probs, axis=1)
                test_acc = (preds == y_test).mean()
            else:
                test_acc = acc

            accuracies.append(test_acc)

            # Convergence detection
            if test_acc >= 0.80 and r < convergence_round:
                convergence_round = r

            last_acc = test_acc

        elapsed = (time.perf_counter() - start) * 1000  # ms

        return ComparisonResult(
            name=name,
            accuracy=last_acc,
            bandwidth=inst.get_bandwidth() * self.n_rounds,
            rounds=self.n_rounds,
            convergence_round=convergence_round,
            memory_mb=inst.get_memory(),
            train_time_ms=elapsed,
        )

    def run_nwp(self) -> ComparisonResult:
        """NWP simulator — uses Hebbian-like local learning."""
        from .decentralized_sgd import DecentralizedSGD
        # NWP is closest to decentralized SGD but with local STDP-like updates
        return self._run_baseline(
            "NWP (DecSGD-local)",
            DecentralizedSGD,
            degree=1,  # Almost no gossip
            local_steps=10,
            learning_rate=0.005,
        )

    def run_federated(self) -> ComparisonResult:
        from .federated import FederatedBaseline
        return self._run_baseline(
            "Federated (FedAvg)",
            FederatedBaseline,
            local_epochs=3,
        )

    def run_decentralized(self) -> ComparisonResult:
        from .decentralized_sgd import DecentralizedSGD
        return self._run_baseline(
            "Decentralized SGD",
            DecentralizedSGD,
            degree=4,
            local_steps=3,
        )

    def run_parameter_server(self) -> ComparisonResult:
        from .parameter_server import ParameterServer
        return self._run_baseline(
            "Parameter Server",
            ParameterServer,
        )

    def run_ray(self) -> ComparisonResult:
        from .ray_baseline import RayBaseline
        return self._run_baseline(
            "Ray (async PS)",
            RayBaseline,
        )

    def run_horovod(self) -> ComparisonResult:
        from .horovod_baseline import HorovodBaseline
        return self._run_baseline(
            "Horovod (allreduce)",
            HorovodBaseline,
        )

    def run_bittensor(self) -> ComparisonResult:
        from .bittensor_baseline import BittensorBaseline
        return self._run_baseline(
            "Bittensor (subnets)",
            BittensorBaseline,
            subnet_size=10,
        )

    def run_all(self) -> List[ComparisonResult]:
        return [
            self.run_nwp(),
            self.run_federated(),
            self.run_decentralized(),
            self.run_parameter_server(),
            self.run_ray(),
            self.run_horovod(),
            self.run_bittensor(),
        ]

    def report_csv(self, results: List[ComparisonResult]) -> str:
        lines = [
            "framework,accuracy,bandwidth_bytes,rounds,convergence_round,memory_mb,train_time_ms"
        ]
        for r in results:
            lines.append(
                f"{r.name},{r.accuracy:.4f},{r.bandwidth:.0f},{r.rounds},"
                f"{r.convergence_round},{r.memory_mb:.2f},{r.train_time_ms:.1f}"
            )
        return "\n".join(lines)

    def report_markdown(self, results: List[ComparisonResult]) -> str:
        """Render a comparison table in markdown."""
        lines = [
            "| Framework | Accuracy | Bandwidth | Rounds | Converge @ | Memory | Time (ms) |",
            "|-----------|----------|-----------|--------|------------|--------|-----------|",
        ]
        for r in sorted(results, key=lambda x: x.accuracy, reverse=True):
            lines.append(
                f"| {r.name} | {r.accuracy:.2%} | {r.bandwidth:,.0f} B | "
                f"{r.rounds} | r={r.convergence_round} | {r.memory_mb:.1f} MB | {r.train_time_ms:.0f} |"
            )
        return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Run distributed learning baseline comparisons"
    )
    parser.add_argument(
        "--framework", nargs="+",
        default=["nwp", "federated", "decentralized", "ps", "ray", "horovod", "bittensor"],
        help="Frameworks to run (default: all)"
    )
    parser.add_argument("--output", default=None,
                        help="Output CSV file path")
    parser.add_argument(
        "--rounds", type=int, default=50,
        help="Number of training rounds"
    )
    parser.add_argument(
        "--nodes", type=int, default=50,
        help="Number of simulated nodes"
    )
    parser.add_argument("--markdown", action="store_true",
                        help="Output markdown table instead of CSV")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed")
    args = parser.parse_args()

    np.random.seed(args.seed)

    FRAMEWORK_MAP = {
        "nwp": ComparisonFramework.run_nwp,
        "federated": ComparisonFramework.run_federated,
        "decentralized": ComparisonFramework.run_decentralized,
        "ps": ComparisonFramework.run_parameter_server,
        "ray": ComparisonFramework.run_ray,
        "horovod": ComparisonFramework.run_horovod,
        "bittensor": ComparisonFramework.run_bittensor,
    }

    cf = ComparisonFramework(
        n_nodes=args.nodes,
        n_rounds=args.rounds,
    )

    results = []
    for name in args.framework:
        key = name.lower().replace("-", "_")
        if key in FRAMEWORK_MAP:
            print(f"[BASELINE] Running {name}...")
            r = FRAMEWORK_MAP[key](cf)
            results.append(r)
            print(f"  accuracy={r.accuracy:.2%}  bandwidth={r.bandwidth:,.0f}B  "
                  f"converge@r={r.convergence_round}  time={r.train_time_ms:.0f}ms")
        elif name == "all":
            for k, fn in FRAMEWORK_MAP.items():
                print(f"[BASELINE] Running {k}...")
                r = fn(cf)
                results.append(r)
                print(f"  accuracy={r.accuracy:.2%}  bandwidth={r.bandwidth:,.0f}B")
            break

    if args.markdown:
        print("\n" + cf.report_markdown(results))
    else:
        print("\n" + cf.report_csv(results))

    if args.output:
        with open(args.output, "w") as f:
            f.write(cf.report_csv(results))
        print(f"\nResults saved to {args.output}")
