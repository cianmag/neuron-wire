#!/usr/bin/env python3
"""Analyze RQ4 scaling benchmark CSV and produce website data."""

import csv, json, math, sys, os
from pathlib import Path
from collections import defaultdict

def mean(v): return sum(v)/len(v) if v else 0.0
def stdev(v):
    if len(v) < 2: return 0.0
    m = mean(v); return math.sqrt(sum((x-m)**2 for x in v)/(len(v)-1))
def ci95(v):
    n = len(v)
    if n < 2: return (0,0)
    m, se = mean(v), stdev(v)/math.sqrt(n)
    z = {5:2.78,10:2.26,15:2.14,20:2.09,25:2.06}.get(n, 1.96)
    return (m-z*se, m+z*se)

csv_path = Path("results/bench-scale/scaling_results.csv")
if not csv_path.exists():
    print(f"ERROR: {csv_path} not found", file=sys.stderr)
    sys.exit(1)

rows = []
with open(csv_path) as f:
    for r in csv.DictReader(f):
        rows.append(r)

grouped = defaultdict(list)
for r in rows:
    grouped[int(r['node_count'])].append(r)

print("=" * 72)
print("  RQ4 — INFRASTRUCTURE SCALING (LIVE DATA)")
print("=" * 72)
print(f"\n  Total: {len(rows)} trials\n")

results = []
for nc in sorted(grouped.keys()):
    trials = grouped[nc]
    n = len(trials)
    
    successful = [r for r in trials if r['converged'] == 'true']
    conv_times = [float(r['convergence_time_s']) for r in successful
                  if r['convergence_time_s'] != 'NULL']
    bandwidths = [float(r['bandwidth_kbps']) for r in trials]
    avg_peers_list = [float(r['avg_peers']) for r in trials]
    max_peers_list = [int(r['max_peers']) for r in trials]
    pkts_list = [int(r['packets_recv']) for r in trials]
    
    conv_rate = len(successful)/n*100
    mc = mean(conv_times) if conv_times else 0
    sc = stdev(conv_times) if conv_times else 0
    ci = ci95(conv_times) if conv_times else (0,0)
    mb = mean(bandwidths)
    sb = stdev(bandwidths)
    mp = mean(avg_peers_list)
    
    print(f"  {nc:2d} nodes × {n:2d} trials: {conv_rate:5.1f}% converge")
    print(f"    Conv time: {mc:6.2f}s ± {sc:.2f}s  [95% CI: {ci[0]:.2f}–{ci[1]:.2f}s]")
    print(f"    Bandwidth: {mb:8.1f} ± {sb:6.1f} kbps")
    print(f"    Peers:     {mp:.1f}/{nc-1} avg  (max: {max(max_peers_list)})")
    print(f"    Packets:   {mean(pkts_list):7.0f} avg")
    print()
    
    results.append({
        "nodes": nc, "trials": n,
        "converged": len(successful),
        "conv_rate": f"{len(successful)}/{n}",
        "ct_mean": round(mc,2), "ct_std": round(sc,3),
        "ct_min": round(min(conv_times),1) if conv_times else 0,
        "ct_max": round(max(conv_times),1) if conv_times else 0,
        "bw_mean": round(mb,1), "bw_std": round(sb,1),
        "ap_mean": round(mean([float(r['apoptosis_deaths']) for r in trials]),2),
        "mp_mean": round(mean([float(r['max_peers']) for r in trials]),1),
        "pkts_mean": round(mean(pkts_list)),
    })

# Output as JS
print("// ── Copy this into dashboard/index.html ──")
print(f"const DATA = {json.dumps({'scales': results, 'total_trials': len(rows)}, indent=2)};")
print()

# Also save JSON
out_path = csv_path.parent / "scaling_stats.json"
with open(out_path, 'w') as f:
    json.dump(results, f, indent=2)
print(f"Saved: {out_path}")
