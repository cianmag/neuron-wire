#!/usr/bin/env python3
"""Synchronous Parameter Server baseline.

Workers push gradients to server, server updates model, workers pull.
"""
import numpy as np
from typing import List


class ParameterServer:
    """Synchronous parameter server distributed learning."""

    def __init__(self, n_nodes: int, n_features: int, n_classes: int,
                 learning_rate: float = 0.01, batch_size: int = 32):
        self.n_workers = n_nodes
        self.n_classes = n_classes
        self.lr = learning_rate
        self.batch_size = batch_size

        # Server model
        self.weights = np.random.randn(n_features, n_classes).astype(np.float32) * 0.01
        self.bias = np.zeros(n_classes, dtype=np.float32)

    def softmax(self, logits: np.ndarray) -> np.ndarray:
        exps = np.exp(logits - logits.max(axis=1, keepdims=True))
        return exps / exps.sum(axis=1, keepdims=True)

    def train(self, X: np.ndarray, y: np.ndarray) -> float:
        """One sync round: workers compute gradients; server aggregates."""
        splits = np.array_split(np.arange(len(X)), self.n_workers)
        all_grads_w = []
        all_grads_b = []

        for worker_idx in range(self.n_workers):
            idx = splits[worker_idx]
            if len(idx) < 2:
                continue
            X_local, y_local = X[idx], y[idx]

            logits = X_local @ self.weights + self.bias
            probs = self.softmax(logits)
            y_onehot = np.zeros((len(y_local), self.n_classes))
            y_onehot[np.arange(len(y_local)), y_local] = 1.0

            grad_w = X_local.T @ (probs - y_onehot) / len(y_local)
            grad_b = (probs - y_onehot).mean(axis=0)
            all_grads_w.append(grad_w)
            all_grads_b.append(grad_b)

        # Server aggregates gradients (average)
        avg_grad_w = np.mean(all_grads_w, axis=0)
        avg_grad_b = np.mean(all_grads_b, axis=0)

        # Server update
        self.weights -= self.lr * avg_grad_w
        self.bias -= self.lr * avg_grad_b

        # Evaluate
        logits = X @ self.weights + self.bias
        preds = np.argmax(self.softmax(logits), axis=1)
        return (preds == y).mean()

    def get_bandwidth(self) -> float:
        """Bytes per round (gradients from each worker + reply)."""
        grad_bytes = self.weights.nbytes + self.bias.nbytes
        return grad_bytes * self.n_workers * 2  # push + pull

    def get_memory(self) -> float:
        return (self.weights.nbytes + self.bias.nbytes) / 1e6
