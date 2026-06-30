#!/usr/bin/env python3
"""Ray baseline stub for distributed training comparison.

Uses @ray.remote pattern for asynchronous parameter-server style training.
Requires 'ray' package: pip install ray
"""
import numpy as np
from typing import List, Tuple


class RayBaseline:
    """Ray-based distributed training stub.

    This is a reference implementation of the @ray.remote pattern.
    Actual Ray requires `import ray` and `ray.init()`.
    """

    def __init__(self, n_nodes: int, n_features: int, n_classes: int,
                 learning_rate: float = 0.01, batch_size: int = 32):
        self.n_workers = n_nodes
        self.n_features = n_features
        self.n_classes = n_classes
        self.lr = learning_rate
        self.batch_size = batch_size

        # Central model (simulates Ray object store)
        self.weights = np.random.randn(n_features, n_classes).astype(np.float32) * 0.01
        self.bias = np.zeros(n_classes, dtype=np.float32)

    def softmax(self, logits: np.ndarray) -> np.ndarray:
        exps = np.exp(logits - logits.max(axis=1, keepdims=True))
        return exps / exps.sum(axis=1, keepdims=True)

    def _worker_compute_grad(self, X_batch: np.ndarray, y_batch: np.ndarray,
                              w: np.ndarray, b: np.ndarray) -> Tuple[np.ndarray, np.ndarray]:
        """Simulates a @ray.remote task."""
        logits = X_batch @ w + b
        probs = self.softmax(logits)
        y_onehot = np.zeros((len(y_batch), self.n_classes))
        y_onehot[np.arange(len(y_batch)), y_batch] = 1.0
        grad_w = X_batch.T @ (probs - y_onehot) / len(y_batch)
        grad_b = (probs - y_onehot).mean(axis=0)
        return grad_w, grad_b

    def train(self, X: np.ndarray, y: np.ndarray) -> float:
        """One async round: fan-out gradients to workers, sync aggregate."""
        splits = np.array_split(np.arange(len(X)), self.n_workers)
        grads_w, grads_b = [], []

        for idx in splits:
            if len(idx) < 2:
                continue
            gw, gb = self._worker_compute_grad(X[idx], y[idx],
                                                self.weights, self.bias)
            grads_w.append(gw)
            grads_b.append(gb)

        avg_gw = np.mean(grads_w, axis=0)
        avg_gb = np.mean(grads_b, axis=0)
        self.weights -= self.lr * avg_gw
        self.bias -= self.lr * avg_gb

        logits = X @ self.weights + self.bias
        preds = np.argmax(self.softmax(logits), axis=1)
        return (preds == y).mean()

    def get_bandwidth(self) -> float:
        model_bytes = self.weights.nbytes + self.bias.nbytes
        return model_bytes * self.n_workers * 2

    def get_memory(self) -> float:
        return (self.weights.nbytes + self.bias.nbytes) / 1e6
