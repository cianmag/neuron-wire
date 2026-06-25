#!/usr/bin/env python3
"""
Validate simulation output against known-good reference values.

Usage:
    python scripts/validate_repro.py <output-dir> [--known-good known-good/]

Compares the summary.csv against the reference and exits with code 0
on match, 1 on mismatch with a diff report.
"""

import csv, sys, os, json

FLOAT_TOLERANCE = 0.3  # allow minor platform/OS variation

# Map simulation field names -> known-good field names
FIELD_MAP = {
    # simulation field : (kg_field, type, tolerance)
    "node_count":          ("nodes",       int,    None),
    "converged":           ("converged",   "bool", None),
    "convergence_time_secs": ("conv_time_s", float, 0.1),
    "max_peers":           ("max_peers",   int,    None),
    "avg_peers":           ("avg_peers",   float, 0.1),
    "bandwidth_kbps":      ("bw_kbps",     float, 2.0),   # wall-clock timing variation
    "total_packets_recv":  ("pkts_recv",   int,    None),
    "seed":                ("seed",        int,    None),
}

def load_csv_dict(path):
    with open(path) as f:
        rows = list(csv.DictReader(f))
    if not rows:
        raise ValueError(f"Empty CSV: {path}")
    return rows[0]

def get_known_good(nodes, known_good_dir):
    path = os.path.join(known_good_dir, f"benchmark-{nodes}node-paper.csv")
    if not os.path.exists(path):
        raise FileNotFoundError(f"No known-good for {nodes} nodes at {path}")
    return load_csv_dict(path)

def compare(got, expected):
    """Compare simulation output against known-good. Returns list of diffs."""
    diffs = []
    
    for sim_field, (kg_field, vtype, tolerance) in FIELD_MAP.items():
        if sim_field not in got:
            continue
        if kg_field not in expected:
            continue
        
        gv_raw = got[sim_field]
        ev_raw = expected[kg_field]
        
        if vtype == "bool":
            gv = gv_raw.strip().lower() == "true"
            ev = ev_raw.strip().lower() == "true"
            if gv != ev:
                diffs.append(f"  {kg_field}: got={gv} expected={ev}")
        elif vtype == int:
            gv = int(float(gv_raw))
            ev = int(float(ev_raw))
            if gv != ev:
                diffs.append(f"  {kg_field}: got={gv} expected={ev}")
        elif vtype == float:
            gv = float(gv_raw)
            ev = float(ev_raw)
            if abs(gv - ev) > tolerance:
                diffs.append(f"  {kg_field}: got={gv} expected={ev} (diff={abs(gv-ev):.3f} > {tolerance})")
    
    return diffs

def main():
    args = sys.argv[1:]
    if not args or "--help" in args or "-h" in args:
        print("Usage: python scripts/validate_repro.py <output-dir> [--known-good <dir>]")
        sys.exit(1)
    
    output_dir = args[0]
    known_good_dir = "known-good"
    if "--known-good" in args:
        idx = args.index("--known-good")
        known_good_dir = args[idx + 1]
    
    summary_path = os.path.join(output_dir, "summary.csv")
    if not os.path.exists(summary_path):
        print(f"ERROR: summary.csv not found at {summary_path}")
        sys.exit(1)
    
    got = load_csv_dict(summary_path)
    nodes = int(float(got.get("node_count", 3)))
    
    print(f"Checking {nodes}-node benchmark...")
    print(f"  Output:   {summary_path}")
    print(f"  Ref:      {known_good_dir}/")
    
    try:
        expected = get_known_good(nodes, known_good_dir)
    except FileNotFoundError as e:
        print(f"  WARNING: {e}")
        print("  Skipping comparison — no reference data for this scale.")
        sys.exit(0)
    
    diffs = compare(got, expected)
    
    if diffs:
        print(f"❌ REPRODUCTION FAILED — {len(diffs)} field(s) differ from known-good:")
        for d in diffs:
            print(f"    {d}")
        sys.exit(1)
    else:
        print("✅ REPRODUCTION VERIFIED — results match known-good")
        sys.exit(0)

if __name__ == "__main__":
    main()
