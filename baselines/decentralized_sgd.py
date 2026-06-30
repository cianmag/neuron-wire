#!/usr/bin/env python3
"""Decentralized SGD baseline — gossip averaging on a random regular graph.

Each node trains locally, then averages weights with its graph neighbours.
"""
import numpy as np
from typing import List, Set
import random


class DecentralizedSGD:
    """Decentralized SGD with gossip averaging on a random graph."""

    def __init__(self, n_nodes: int, n_features: int, n_classes: int,
                 degree: int = 3, learning_rate: float = 0.01,
                 batch_size: int = 32, local_steps: int = 5):
        self.n_nodes = n_nodes
        self.n_classes = n_classes
        self.lr = learning_rate
        self.batch_size = batch_size
        self.local_steps = local_steps
        self.degree = min(degree, n_nodes - 1)

        # Build random regular-ish graph
        self.adj: List[Set[int]] = [set() for _ in range(n_nodes)]
        for i in range(n_nodes):
            candidates = [j for j in range(n_nodes) if j != i and j not in self.adj[i]]
            random.shuffle(candidates)
            needed = self.degree - len(self.adj[i])
            for j in candidates[:needed]:
                self.adj[i].add(j)
                self.adj[j].add(i)

        # Per-node model parameters
        self.node_weights = [
            np.random.randn(n_features, n_classes).astype(np.float32) * 0.01
            for _ in range(n_nodes)
        ]
        self.node_biases = [
            np.zeros(n_classes, dtype=np.float32) for _ in range(n_nodes)
        ]

    def softmax(self, logits: np.ndarray) -> np.ndarray:
        exps = np.exp(logits - logits.max(axis=1, keepdims=True))
        return exps / exps.sum(axis=1, keepdims=True)

    def train(self, X: np.ndarray, y: np.ndarray) -> float:
        """Run one decentralized training step: local SGD + gossip averaging."""
        splits = np.array_split(np.arange(len(X)), self.n_nodes)

        for node_idx in range(self.n_nodes):
            idx = splits[node_idx]
            if len(idx) < 2:
                continue
            X_local, y_local = X[idx], y[idx]

            w = self.node_weights[node_idx]
            b = self.node_biases[node_idx]

            for _ in range(self.local_steps):
                perm = np.random.permutation(len(X_local))
                for start in range(0, len(X_local), self.batch_size):
                    batch_idx = perm[start:start + self.batch_size]
                    X_b = X_local[batch_idx]
                    y_b = y_local[batch_idx]

                    logits = X_b @ w + b
                    probs = self.softmax(logits)
                    y_onehot = np.zeros((len(y_b), self.n_classes))
                    y_onehot[np.arange(len(y_b)), y_b] = 1.0

                    grad = X_b.T @ (probs - y_onehot) / len(y_b)
                    grad_b = (probs - y_onehot).mean(axis=0)
                    w -= self.lr * grad
                    b -= self.lr * grad_b

            # Gossip: average with neighbours
            neighbours = list(self.adj[node_idx])
            if neighbours:
                w_avg = w.copy()
                b_avg = b.copy()
                for nb in neighbours:
                    w_avg += self.node_weights[nb]
                    b_avg += self.node_biases[nb]
                scale = 1.0 / (1 + len(neighbours))
                self.node_weights[node_idx] = w_avg * scale
                self.node_biases[node_idx] = b_avg * scale

        # Global consensus model = average of all nodes
        global_w = np.mean(self.node_weights, axis=0)
        global_b = np.mean(self.node_biases, axis=0)
        logits = X @ global_w + global_b
        preds = np.argmax(self.softmax(logits), axis=1)
        return (preds == y).mean()

    def get_bandwidth(self) -> float:
        """Bytes exchanged per round (degree * 2 * model_size)."""
        model_bytes = self.node_weights[0].nbytes + self.node_biases[0].nbytes
        return model_bytes * self.degree * 2  # send + receive per neighbour

    def get_memory(self) -> float:
        """MB per node."""
        return (self.node_weights[0].nbytes + self.node_biases[0].nbytes) / 1e6
