#!/usr/bin/env python3
"""Aggregate NWP evidence-matrix results into a master table.

Walks <out>/**/summary.csv (produced by examples/simulate.rs in paper mode),
merges every trial into one master CSV plus a Markdown table suitable for
the Evidence Report.

Usage:
    python3 evidence/aggregate.py results/evidence
Output:
    <out>/evidence_master.csv
    <out>/evidence_table.md
"""
import csv
import glob
import os
import sys


def main() -> int:
    root = sys.argv[1] if len(sys.argv) > 1 else "results/evidence"
    rows = []
    for path in sorted(glob.glob(os.path.join(root, "**", "summary.csv"), recursive=True)):
        experiment = os.path.relpath(os.path.dirname(path), root)
        with open(path, newline="") as f:
            reader = csv.DictReader(f)
            for r in reader:
                r["experiment"] = experiment
                rows.append(r)

    if not rows:
        print(f"[aggregate] no summary.csv found under {root}")
        return 1

    fields = list(rows[0].keys())
    master = os.path.join(root, "evidence_master.csv")
    with open(master, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        w.writerows(rows)
    print(f"[aggregate] {len(rows)} trials → {master}")

    # Markdown table
    md = os.path.join(root, "evidence_table.md")
    cols = ["experiment", "seed", "node_count", "converged", "convergence_time_secs",
            "avg_peers", "max_peers", "total_packets_sent", "total_bytes_sent",
            "bandwidth_kbps", "failure_mode", "recovered", "recovery_time_secs",
            "total_apoptosis_deaths"]
    with open(md, "w") as f:
        f.write("| " + " | ".join(cols) + " |\n")
        f.write("|" + "|".join(["---"] * len(cols)) + "|\n")
        for r in rows:
            f.write("| " + " | ".join(r.get(c, "") or "" for c in cols) + " |\n")
    print(f"[aggregate] table → {md}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
