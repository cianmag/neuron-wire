#!/usr/bin/env python3
"""generate_figures.py — Generate publication-quality figures from experiment CSVs.

Usage:
    python3 scripts/generate_figures.py --input-dir results/ --output-dir results/figures/ --format png

Outputs:
    figures/
    ├── figure-1-dht-convergence-3node.png   (or .pdf)
    ├── figure-2-dht-convergence-10node.png
    ├── figure-3-convergence-time-scaling.png
    ├── figure-4-bandwidth-vs-nodes.png
    ├── figure-5-node-churn-recovery.png
    ├── figure-6-failure-injection.png
    ├── figure-7-multi-trial-stats.png
    ├── figure-8-sga-comparison.png
    └── figure-9-scaling-summary.png          (multi-panel overview)
"""

import argparse
import csv
import json
import os
import sys
from collections import defaultdict
from pathlib import Path

# Optional dependencies — provide helpful error if missing
try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker as mticker
    import numpy as np
except ImportError:
    print("Required: matplotlib, numpy. Install with: pip install -r scripts/requirements.txt")
    sys.exit(1)

# ── Style ──────────────────────────────────────────────────────────────────
plt.style.use("seaborn-v0_8-whitegrid")
COLORS = {
    "primary": "#0055CC",
    "secondary": "#00AA66",
    "accent": "#CC4400",
    "comparison": ["#0055CC", "#CC4400", "#00AA66", "#AA44CC"],
}
MARKERS = ["o", "s", "D", "^"]

def setup_figure(title, xlabel, ylabel, figsize=(8, 5)):
    """Create a styled figure with common settings."""
    fig, ax = plt.subplots(figsize=figsize)
    ax.set_title(title, fontsize=14, fontweight="bold", pad=12)
    ax.set_xlabel(xlabel, fontsize=11)
    ax.set_ylabel(ylabel, fontsize=11)
    ax.tick_params(labelsize=10)
    ax.grid(True, alpha=0.3)
    return fig, ax

def save_figure(fig, output_dir, name, fmt="png"):
    """Save figure to output directory."""
    path = Path(output_dir) / f"{name}.{fmt}"
    fig.savefig(path, dpi=150, bbox_inches="tight", facecolor="white")
    print(f"  Saved: {path}")
    plt.close(fig)

def read_csv_safe(path):
    """Read CSV, return list of dicts. Handles missing files gracefully."""
    if not Path(path).exists():
        print(f"  (skipped: {path} not found)")
        return None
    with open(path) as f:
        reader = csv.DictReader(f)
        return list(reader)

# ── Figure generators ──────────────────────────────────────────────────────

def figure_1_dht_convergence_3node(input_dir, output_dir, fmt):
    """Figure 1: 3-node DHT convergence timeline."""
    data = read_csv_safe(Path(input_dir) / "figure-1-dht-convergence-3node" / "convergence.csv")
    if not data:
        return

    fig, ax = setup_figure(
        "Figure 1: DHT Convergence — 3 Nodes",
        "Time (seconds)",
        "Peers Discovered"
    )

    # Group by node
    nodes = defaultdict(list)
    for row in data:
        node = row.get("node_id", "unknown")
        nodes[node].append({
            "time": float(row.get("time_s", 0)),
            "peers": int(row.get("peer_count", 0)),
        })

    for i, (node, pts) in enumerate(nodes.items()):
        pts.sort(key=lambda x: x["time"])
        times = [p["time"] for p in pts]
        peers = [p["peers"] for p in pts]
        label = node[-8:] if len(node) > 8 else node
        ax.step(times, peers, where="post", label=f"Node {label}",
                color=COLORS["comparison"][i % 4], marker=MARKERS[i % 4], markersize=4, alpha=0.8)

    ax.set_ylim(bottom=-0.5, top=2.5)
    ax.yaxis.set_major_locator(mticker.MaxNLocator(integer=True))
    ax.legend(fontsize=9)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-1-dht-convergence-3node", fmt)


def figure_2_dht_convergence_10node(input_dir, output_dir, fmt):
    """Figure 2: 10-node DHT convergence scaling."""
    data = read_csv_safe(Path(input_dir) / "figure-2-dht-convergence-10node" / "summary.csv")
    if not data:
        return

    fig, ax = setup_figure(
        "Figure 2: DHT Convergence Scaling — 10 Nodes",
        "Trial",
        "Convergence Time (s)"
    )

    trials = [int(r.get("trial", i)) for i, r in enumerate(data)]
    conv_times = [float(r.get("conv_time_s", 0)) for r in data]
    ax.bar(trials, conv_times, color=COLORS["primary"], alpha=0.7, edgecolor="white")
    ax.set_xticks(trials)
    ax.set_xticklabels([str(t) for t in trials])

    mean_t = np.mean(conv_times)
    ax.axhline(mean_t, color=COLORS["accent"], linestyle="--", alpha=0.8,
               label=f"Mean: {mean_t:.1f}s")
    ax.legend(fontsize=9)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-2-dht-convergence-10node", fmt)


def figure_3_convergence_time_scaling(input_dir, output_dir, fmt):
    """Figure 3: Convergence time vs node count."""
    # Collect from all figure directories
    node_counts = []
    mean_times = []
    std_times = []

    for d in sorted(Path(input_dir).glob("figure-*")):
        summary = d / "summary.csv"
        if not summary.exists():
            continue
        data = read_csv_safe(summary)
        if not data:
            continue

        # Try to extract node count from config or data
        config_path = d / "experiment.toml"
        node_count = None
        if config_path.exists():
            try:
                import toml
                cfg = toml.load(config_path)
                node_count = cfg.get("simulation", {}).get("node_count")
            except Exception:
                pass
        if node_count is None and data:
            node_count = int(data[0].get("node_count", 0))

        if node_count and data:
            node_counts.append(node_count)
            times = [float(r.get("conv_time_s", 0)) for r in data if r.get("conv_time_s")]
            if times:
                mean_times.append(np.mean(times))
                std_times.append(np.std(times))

    if not node_counts:
        print("  (skipped: no convergence data across multiple node counts)")
        return

    fig, ax = setup_figure(
        "Figure 3: Convergence Time Scaling",
        "Nodes",
        "Convergence Time (s)"
    )

    ax.errorbar(node_counts, mean_times, yerr=std_times,
                fmt="o-", color=COLORS["primary"], capsize=5, capthick=1.5,
                markersize=8, linewidth=2, label="Measured")

    # Fit O(log n) reference curve
    if len(node_counts) >= 3:
        xs = np.linspace(min(node_counts), max(node_counts), 100)
        c = np.mean([t / np.log(n) for t, n in zip(mean_times, node_counts) if n > 1])
        ax.plot(xs, c * np.log(xs), "--", color=COLORS["accent"], alpha=0.5,
                label="O(log n) reference")

    ax.set_xscale("log")
    ax.legend(fontsize=9)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-3-convergence-time-scaling", fmt)


def figure_4_bandwidth_scaling(input_dir, output_dir, fmt):
    """Figure 4: Bandwidth scaling with network size."""
    node_counts = []
    bandwidths = []

    for d in sorted(Path(input_dir).glob("figure-*")):
        summary = d / "summary.csv"
        if not summary.exists():
            continue
        data = read_csv_safe(summary)
        if not data:
            continue
        config_path = d / "experiment.toml"
        node_count = None
        if config_path.exists():
            try:
                import toml
                cfg = toml.load(config_path)
                node_count = cfg.get("simulation", {}).get("node_count")
            except Exception:
                pass

        if node_count and data:
            node_counts.append(node_count)
            bw = [float(r.get("bandwidth_kbps", 0)) for r in data]
            if bw:
                bandwidths.append(np.mean(bw))

    if not node_counts:
        print("  (skipped: no bandwidth data)")
        return

    fig, ax = setup_figure(
        "Figure 4: Bandwidth Scaling",
        "Nodes",
        "Bandwidth (kbps)"
    )

    ax.plot(node_counts, bandwidths, "s-", color=COLORS["secondary"],
            markersize=8, linewidth=2, label="Total bandwidth")

    if len(node_counts) >= 3:
        xs = np.linspace(min(node_counts), max(node_counts), 100)
        p = np.polyfit(np.log(node_counts), bandwidths, 1)
        ax.plot(xs, np.polyval(p, np.log(xs)), "--", color=COLORS["accent"],
                alpha=0.5, label=f"O(log n) fit")

    ax.set_xscale("log")
    ax.legend(fontsize=9)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-4-bandwidth-scaling", fmt)


def figure_5_node_churn_recovery(input_dir, output_dir, fmt):
    """Figure 5: Node churn and recovery."""
    data = read_csv_safe(Path(input_dir) / "figure-6-node-churn-recovery" / "routing.csv")
    if not data:
        print("  (skipped: no churn data)")
        return

    fig, ax = setup_figure(
        "Figure 5: Node Churn Recovery",
        "Time (s)",
        "Active Peers"
    )

    nodes = defaultdict(list)
    for row in data:
        node = row.get("node_id", "unknown")
        nodes[node].append({
            "time": float(row.get("time_s", 0)),
            "peers": int(row.get("peer_count", 0)),
        })

    for node, pts in sorted(nodes.items()):
        pts.sort(key=lambda x: x["time"])
        times = [p["time"] for p in pts]
        peers = [p["peers"] for p in pts]
        label = node[-8:] if len(node) > 8 else node
        ax.plot(times, peers, "-", label=label, alpha=0.7)

    # Mark failure injection point (15s)
    ax.axvline(15, color=COLORS["accent"], linestyle="--", alpha=0.6, label="Failure @ 15s")
    ax.legend(fontsize=8, ncol=2)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-5-node-churn-recovery", fmt)


def figure_6_failure_injection(input_dir, output_dir, fmt):
    """Figure 6: Failure injection comparison."""
    data = read_csv_safe(Path(input_dir) / "figure-7-failure-injection" / "summary.csv")
    if not data:
        print("  (skipped: no failure injection data)")
        return

    fig, ax = setup_figure(
        "Figure 6: Failure Injection — 20% Packet Loss",
        "Trial",
        "Packets Delivered",
        figsize=(8, 4)
    )

    trials = [int(r.get("trial", i)) for i, r in enumerate(data)]
    pkts = [int(float(r.get("total_packets_recv", 0))) for r in data]
    bars = ax.bar(trials, pkts, color=[COLORS["primary"]] * len(trials), alpha=0.7,
                  edgecolor="white")

    ax.set_xticks(trials)
    ax.set_xticklabels([str(t) for t in trials])
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-6-failure-injection", fmt)


def figure_7_multi_trial_stats(input_dir, output_dir, fmt):
    """Figure 7: Multi-trial statistical distribution."""
    data = read_csv_safe(Path(input_dir) / "figure-8-multi-trial-stats" / "summary.csv")
    if not data:
        print("  (skipped: no multi-trial data)")
        return

    fig, axes = plt.subplots(1, 2, figsize=(12, 4.5))

    # Metric 1: Convergence time
    ax = axes[0]
    conv_times = [float(r.get("conv_time_s", 0)) for r in data if r.get("conv_time_s")]
    if conv_times:
        ax.boxplot(conv_times, vert=True, patch_artist=True,
                   boxprops=dict(facecolor=COLORS["primary"], alpha=0.4),
                   medianprops=dict(color=COLORS["accent"], linewidth=2))
        ax.set_title("Convergence Time Distribution", fontsize=12, fontweight="bold")
        ax.set_ylabel("Seconds")
        ax.set_xticks([1])
        ax.set_xticklabels([f"n={len(conv_times)}"])

    # Metric 2: Bandwidth
    ax = axes[1]
    bw = [float(r.get("bandwidth_kbps", 0)) for r in data]
    if bw:
        ax.boxplot(bw, vert=True, patch_artist=True,
                   boxprops=dict(facecolor=COLORS["secondary"], alpha=0.4),
                   medianprops=dict(color=COLORS["accent"], linewidth=2))
        ax.set_title("Bandwidth Distribution", fontsize=12, fontweight="bold")
        ax.set_ylabel("kbps")
        ax.set_xticks([1])
        ax.set_xticklabels([f"n={len(bw)}"])

    fig.suptitle("Figure 7: Multi-Trial Statistics (10 trials)", fontsize=14, fontweight="bold", y=1.02)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-7-multi-trial-stats", fmt)


def figure_8_sga_comparison(input_dir, output_dir, fmt):
    """Figure 8: SGA vs Fixed gossip comparison."""
    data = read_csv_safe(Path(input_dir) / "figure-10-sga-comparison" / "summary.csv")
    if not data:
        print("  (skipped: no SGA data)")
        return

    fig, ax = setup_figure(
        "Figure 8: Sparse Gossip Aging vs Fixed Fanout",
        "Metric",
        "Value (normalized)"
    )

    # Compare metrics
    metrics = ["bandwidth_kbps", "conv_time_s"]
    labels = ["Bandwidth (kbps)", "Convergence Time (s)"]
    values = []
    for m in metrics:
        vals = [float(r.get(m, 0)) for r in data if r.get(m)]
        if vals:
            values.append(np.mean(vals))

    if values:
        x = np.arange(len(labels))
        ax.bar(x, values, color=COLORS["comparison"][:len(values)], alpha=0.7, edgecolor="white")
        ax.set_xticks(x)
        ax.set_xticklabels(labels, fontsize=10)

    fig.tight_layout()
    save_figure(fig, output_dir, "figure-8-sga-comparison", fmt)


def figure_9_scaling_summary(input_dir, output_dir, fmt):
    """Figure 9: Multi-panel scaling summary."""
    # Collect data from all figure directories
    node_data = {}
    for d in sorted(Path(input_dir).glob("figure-*")):
        summary = d / "summary.csv"
        if not summary.exists():
            continue
        data = read_csv_safe(summary)
        if not data:
            continue
        config_path = d / "experiment.toml"
        if config_path.exists():
            try:
                import toml
                cfg = toml.load(config_path)
                nc = cfg.get("simulation", {}).get("node_count")
                if nc and data:
                    node_data[nc] = data
            except Exception:
                pass

    if not node_data:
        print("  (skipped: insufficient data for summary)")
        return

    fig, axes = plt.subplots(2, 2, figsize=(12, 8))

    # Panel 1: Convergence rate
    ax = axes[0, 0]
    ncs = sorted(node_data.keys())
    rates = []
    for nc in ncs:
        conv = sum(1 for r in node_data[nc] if r.get("converged", "").lower() == "true")
        total = len(node_data[nc])
        rates.append(conv / total * 100 if total else 0)
    ax.bar([str(n) for n in ncs], rates, color=COLORS["primary"], alpha=0.7)
    ax.set_title("Convergence Rate", fontsize=12, fontweight="bold")
    ax.set_ylabel("Percent")
    ax.set_ylim(0, 105)

    # Panel 2: Convergence time
    ax = axes[0, 1]
    times = []
    for nc in ncs:
        t = [float(r.get("conv_time_s", 0)) for r in node_data[nc] if r.get("conv_time_s")]
        times.append(np.mean(t) if t else 0)
    ax.plot(ncs, times, "o-", color=COLORS["primary"], linewidth=2, markersize=8)
    ax.set_title("Convergence Time", fontsize=12, fontweight="bold")
    ax.set_ylabel("Seconds")

    # Panel 3: Peer discovery
    ax = axes[1, 0]
    peers = []
    for nc in ncs:
        p = [float(r.get("avg_peers", 0)) for r in node_data[nc]]
        peers.append(np.mean(p) if p else 0)
    ax.plot(ncs, peers, "s-", color=COLORS["secondary"], linewidth=2, markersize=8)
    ax.set_title("Peer Discovery", fontsize=12, fontweight="bold")
    ax.set_xlabel("Nodes")
    ax.set_ylabel("Avg Peers")

    # Panel 4: Bandwidth
    ax = axes[1, 1]
    bw = []
    for nc in ncs:
        b = [float(r.get("bandwidth_kbps", 0)) for r in node_data[nc]]
        bw.append(np.mean(b) if b else 0)
    ax.plot(ncs, bw, "D-", color=COLORS["accent"], linewidth=2, markersize=8)
    ax.set_title("Bandwidth Scaling", fontsize=12, fontweight="bold")
    ax.set_xlabel("Nodes")
    ax.set_ylabel("kbps")

    fig.suptitle("Figure 9: Scaling Summary", fontsize=14, fontweight="bold", y=1.02)
    fig.tight_layout()
    save_figure(fig, output_dir, "figure-9-scaling-summary", fmt)


# ── Main ───────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Generate publication figures from experiment CSVs")
    parser.add_argument("--input-dir", default="results", help="Directory with figure subdirectories")
    parser.add_argument("--output-dir", default="results/figures", help="Where to save figures")
    parser.add_argument("--format", default="png", choices=["png", "pdf", "svg"], help="Output format")
    args = parser.parse_args()

    input_dir = Path(args.input_dir)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Generating figures from {input_dir} → {output_dir}/")
    print()

    figure_1_dht_convergence_3node(input_dir, output_dir, args.format)
    figure_2_dht_convergence_10node(input_dir, output_dir, args.format)
    figure_3_convergence_time_scaling(input_dir, output_dir, args.format)
    figure_4_bandwidth_scaling(input_dir, output_dir, args.format)
    figure_5_node_churn_recovery(input_dir, output_dir, args.format)
    figure_6_failure_injection(input_dir, output_dir, args.format)
    figure_7_multi_trial_stats(input_dir, output_dir, args.format)
    figure_8_sga_comparison(input_dir, output_dir, args.format)
    figure_9_scaling_summary(input_dir, output_dir, args.format)

    print()
    print(f"Done — {len(list(output_dir.glob(f'*.{args.format}')))} figures saved to {output_dir}/")


if __name__ == "__main__":
    main()
