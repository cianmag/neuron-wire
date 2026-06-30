#!/usr/bin/env python3
"""Horovod baseline stub — allreduce-based distributed SGD.

Horovod uses ring allreduce for gradient averaging. Requires:
    pip install horovod
    horovodrun -np 4 python horovod_baseline.py
"""
import numpy as np
from typing import List


class HorovodBaseline:
    """Horovod-style allreduce distributed training stub.

    Simulates the hvd.allreduce() pattern without requiring horovod installed.
    """

    def __init__(self, n_nodes: int, n_features: int, n_classes: int,
                 learning_rate: float = 0.01, batch_size: int = 32):
        self.n_workers = n_nodes
        self.n_features = n_features
        self.n_classes = n_classes
        self.lr = learning_rate
        self.batch_size = batch_size

        self.weights = np.random.randn(n_features, n_classes).astype(np.float32) * 0.01
        self.bias = np.zeros(n_classes, dtype=np.float32)

    def softmax(self, logits: np.ndarray) -> np.ndarray:
        exps = np.exp(logits - logits.max(axis=1, keepdims=True))
        return exps / exps.sum(axis=1, keepdims=True)

    def _allreduce(self, grad: np.ndarray) -> np.ndarray:
        """Simulate hvd.allreduce: average gradients across workers."""
        return grad / self.n_workers

    def train(self, X: np.ndarray, y: np.ndarray) -> float:
        """One training step with simulated allreduce."""
        splits = np.array_split(np.arange(len(X)), self.n_workers)
        grads_w, grads_b = [], []

        for idx in splits:
            if len(idx) < 2:
                continue
            X_local, y_local = X[idx], y[idx]
            logits = X_local @ self.weights + self.bias
            probs = self.softmax(logits)
            y_onehot = np.zeros((len(y_local), self.n_classes))
            y_onehot[np.arange(len(y_local)), y_local] = 1.0
            gw = X_local.T @ (probs - y_onehot) / len(y_local)
            gb = (probs - y_onehot).mean(axis=0)
            grads_w.append(gw)
            grads_b.append(gb)

        # hvd.allreduce: average all gradients
        avg_gw = np.mean(grads_w, axis=0)
        avg_gb = np.mean(grads_b, axis=0)

        self.weights -= self.lr * avg_gw
        self.bias -= self.lr * avg_gb

        logits = X @ self.weights + self.bias
        preds = np.argmax(self.softmax(logits), axis=1)
        return (preds == y).mean()

    def get_bandwidth(self) -> float:
        """Allreduce bandwidth: O(log N * model_size)."""
        model_bytes = self.weights.nbytes + self.bias.nbytes
        return model_bytes * int(np.log2(self.n_workers))

    def get_memory(self) -> float:
        return (self.weights.nbytes + self.bias.nbytes) / 1e6
