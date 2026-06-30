#!/usr/bin/env python3
"""Federated Averaging (FedAvg) baseline for distributed learning comparison.

Trains a linear classifier across N clients with FedAvg aggregation.
"""
import numpy as np
from typing import Tuple


class FederatedBaseline:
    """Federated learning with FedAvg aggregation."""

    def __init__(self, n_nodes: int, n_features: int, n_classes: int,
                 local_epochs: int = 5, learning_rate: float = 0.01,
                 batch_size: int = 32):
        self.n_nodes = n_nodes
        self.n_classes = n_classes
        self.local_epochs = local_epochs
        self.lr = learning_rate
        self.batch_size = batch_size
        # Global model: linear classifier weights [n_features x n_classes]
        self.global_weights = np.random.randn(n_features, n_classes).astype(np.float32) * 0.01
        self.global_bias = np.zeros(n_classes, dtype=np.float32)

    def softmax(self, logits: np.ndarray) -> np.ndarray:
        exps = np.exp(logits - logits.max(axis=1, keepdims=True))
        return exps / exps.sum(axis=1, keepdims=True)

    def train(self, X: np.ndarray, y: np.ndarray) -> float:
        """Run one federated round over local epochs on each node."""
        # Partition data among nodes
        splits = np.array_split(np.arange(len(X)), self.n_nodes)
        node_updates = []

        for node_idx in range(self.n_nodes):
            idx = splits[node_idx]
            if len(idx) < 2:
                continue
            X_local, y_local = X[idx], y[idx]

            # Local SGD
            w_local = self.global_weights.copy()
            b_local = self.global_bias.copy()

            for _ in range(self.local_epochs):
                perm = np.random.permutation(len(X_local))
                for start in range(0, len(X_local), self.batch_size):
                    batch_idx = perm[start:start + self.batch_size]
                    X_b = X_local[batch_idx]
                    y_b = y_local[batch_idx]

                    logits = X_b @ w_local + b_local
                    probs = self.softmax(logits)

                    # One-hot target
                    y_onehot = np.zeros((len(y_b), self.n_classes))
                    y_onehot[np.arange(len(y_b)), y_b] = 1.0

                    grad = X_b.T @ (probs - y_onehot) / len(y_b)
                    grad_b = (probs - y_onehot).mean(axis=0)

                    w_local -= self.lr * grad
                    b_local -= self.lr * grad_b

            node_updates.append((w_local, b_local, len(idx)))

        # FedAvg: weighted average of model parameters
        total_samples = sum(n for _, _, n in node_updates)
        if total_samples == 0:
            return 0.0

        self.global_weights = np.sum(
            [w * n for w, _, n in node_updates], axis=0
        ) / total_samples
        self.global_bias = np.sum(
            [b * n for _, b, n in node_updates], axis=0
        ) / total_samples

        # Evaluate
        logits = X @ self.global_weights + self.global_bias
        preds = np.argmax(self.softmax(logits), axis=1)
        return (preds == y).mean()

    def get_bandwidth(self) -> float:
        """Estimate bytes per round (model size * n_nodes)."""
        model_bytes = self.global_weights.nbytes + self.global_bias.nbytes
        return model_bytes * self.n_nodes  # round trip

    def get_memory(self) -> float:
        """Memory in MB for local model."""
        return (self.global_weights.nbytes + self.global_bias.nbytes) / 1e6
