#!/usr/bin/env python3
"""Check Criterion benchmark output for performance regressions >= 5%."""
import re
import sys

BENCH_FILE = "bench_output.txt"

try:
    with open(BENCH_FILE) as f:
        text = f.read()
except FileNotFoundError:
    print(f"::warning::No {BENCH_FILE} found — skipping regression check")
    sys.exit(0)

regressions = 0
changes = 0
for line in text.split("\n"):
    if "Performance has regressed" in line:
        regressions += 1
        print(f"REGRESSION: {line.strip()}")
    m = re.search(r"change:\s*\[(-?\d+(?:\.\d+)?)%,", line)
    if m:
        changes += 1
        pct = float(m.group(1))
        if pct < -5.0:
            regressions += 1
            print(f"REGRESSION ({pct:.0f}%): {line.strip()}")

if regressions > 0:
    print(f"\n❌ {regressions} performance regression(s) — build failing.")
    sys.exit(1)
else:
    print(f"\n✅ No performance regressions (checked {changes} change lines).")
