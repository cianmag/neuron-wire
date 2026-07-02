#!/usr/bin/env bash
# capture_env.sh — Capture full environment metadata for reproducibility.
#
# Outputs to a directory: metadata.json (all fields), and individual text files.
#
# Usage:
#   bash scripts/capture_env.sh <output-dir>

set -euo pipefail

OUTDIR="${1:-./env-capture}"
mkdir -p "$OUTDIR"

# ── Timestamp ────────────────────────────────────────────────────────────
TIMESTAMP=$(date --iso-8601=seconds 2>/dev/null || date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "$TIMESTAMP" > "$OUTDIR/timestamp.txt"

# ── Git info ─────────────────────────────────────────────────────────────
if git rev-parse --git-dir &>/dev/null; then
  GIT_COMMIT=$(git rev-parse HEAD)
  GIT_SHORT=$(git rev-parse --short HEAD)
  GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
  GIT_TAG=$(git describe --tags --exact-match 2>/dev/null || echo "none")
  GIT_DIRTY=$(git status --porcelain | wc -l)
  GIT_REPO=$(git remote get-url origin 2>/dev/null || echo "unknown")
else
  GIT_COMMIT="unknown"; GIT_SHORT="unknown"; GIT_BRANCH="unknown"
  GIT_TAG="unknown"; GIT_DIRTY="unknown"; GIT_REPO="unknown"
fi
echo "$GIT_COMMIT" > "$OUTDIR/commit.txt"
echo "$GIT_BRANCH" > "$OUTDIR/branch.txt"
echo "$GIT_TAG" > "$OUTDIR/tag.txt"
echo "$((GIT_DIRTY)) uncommitted changes" > "$OUTDIR/dirty.txt"
echo "$GIT_REPO" > "$OUTDIR/repo.txt"

# ── Compiler info ────────────────────────────────────────────────────────
rustc --version > "$OUTDIR/rustc.txt" 2>/dev/null || echo "rustc not found" > "$OUTDIR/rustc.txt"
cargo --version > "$OUTDIR/cargo.txt" 2>/dev/null || echo "cargo not found" > "$OUTDIR/cargo.txt"
rustup show active-toolchain > "$OUTDIR/rustup.txt" 2>/dev/null || echo "rustup not found" > "$OUTDIR/rustup.txt"

# ── OS info ──────────────────────────────────────────────────────────────
if [ -f /etc/os-release ]; then
  grep -E "^PRETTY_NAME|^VERSION_ID|^ID" /etc/os-release > "$OUTDIR/os.txt"
elif [ -f /etc/lsb-release ]; then
  cat /etc/lsb-release > "$OUTDIR/os.txt"
elif command -v sw_vers &>/dev/null; then
  sw_vers > "$OUTDIR/os.txt"
elif command -v systeminfo &>/dev/null; then
  systeminfo | head -20 > "$OUTDIR/os.txt"
else
  uname -a > "$OUTDIR/os.txt"
fi
uname -a > "$OUTDIR/kernel.txt"

# ── Hardware info ────────────────────────────────────────────────────────
if [ -f /proc/cpuinfo ]; then
  grep -m1 "model name" /proc/cpuinfo | cut -d: -f2 | sed 's/^ //' > "$OUTDIR/cpu.txt"
  grep -c "^processor" /proc/cpuinfo > "$OUTDIR/cpu_cores.txt"
elif command -v sysctl &>/dev/null; then
  sysctl -n machdep.cpu.brand_string > "$OUTDIR/cpu.txt" 2>/dev/null || echo "unknown" > "$OUTDIR/cpu.txt"
  sysctl -n hw.ncpu > "$OUTDIR/cpu_cores.txt" 2>/dev/null || echo "unknown" > "$OUTDIR/cpu_cores.txt"
else
  echo "unknown" > "$OUTDIR/cpu.txt"
  echo "unknown" > "$OUTDIR/cpu_cores.txt"
fi

# Memory
if [ -f /proc/meminfo ]; then
  grep -m1 "MemTotal" /proc/meminfo > "$OUTDIR/memory.txt"
elif command -v sysctl &>/dev/null; then
  sysctl -n hw.memsize > "$OUTDIR/memory.txt" 2>/dev/null || echo "unknown" > "$OUTDIR/memory.txt"
else
  echo "unknown" > "$OUTDIR/memory.txt"
fi
hostname > "$OUTDIR/hostname.txt" 2>/dev/null || echo "unknown" > "$OUTDIR/hostname.txt"

# ── Dependency tree ──────────────────────────────────────────────────────
if [ -f Cargo.toml ]; then
  cargo tree --prefix depth 2>/dev/null > "$OUTDIR/dependencies.txt" || echo "cargo tree failed" > "$OUTDIR/dependencies.txt"
  # Also grab a compact summary
  cargo tree --prefix none 2>/dev/null | sort -u > "$OUTDIR/dependencies-compact.txt" || true
fi

# ── Hashes of all source files ──────────────────────────────────────────
if command -v sha256sum &>/dev/null; then
  HASH_CMD="sha256sum"
elif command -v shasum &>/dev/null; then
  HASH_CMD="shasum -a 256"
else
  HASH_CMD=""
fi

if [ -n "$HASH_CMD" ]; then
  find src/ -type f -name "*.rs" -exec $HASH_CMD {} \; > "$OUTDIR/source-hashes.txt"
  [ -f Cargo.toml ] && $HASH_CMD Cargo.toml >> "$OUTDIR/source-hashes.txt"
  [ -f Cargo.lock ] && $HASH_CMD Cargo.lock >> "$OUTDIR/source-hashes.txt"
fi

# ── SHA256 of every output file (run at end of experiment) ──────────────
# This function should be called at the end of reproduce.sh
hash_outputs() {
  local dir="$1"
  if [ -n "$HASH_CMD" ]; then
    find "$dir" -type f \( -name "*.csv" -o -name "*.toml" -o -name "*.json" -o -name "*.jsonl" \) \
      -exec $HASH_CMD {} \; > "$dir/output-hashes.txt"
  fi
}

# ── Consolidated JSON ────────────────────────────────────────────────────
python3 -c "
import json, os

def read_file(path):
    try:
        with open(path) as f:
            return f.read().strip()
    except: return None

d = {
    'timestamp': read_file('$OUTDIR/timestamp.txt'),
    'git': {
        'commit': read_file('$OUTDIR/commit.txt'),
        'short': read_file('$OUTDIR/commit.txt')[:7] if read_file('$OUTDIR/commit.txt') else None,
        'branch': read_file('$OUTDIR/branch.txt'),
        'tag': read_file('$OUTDIR/tag.txt'),
        'dirty_files': read_file('$OUTDIR/dirty.txt'),
        'repo': read_file('$OUTDIR/repo.txt'),
    },
    'compiler': {
        'rustc': read_file('$OUTDIR/rustc.txt'),
        'cargo': read_file('$OUTDIR/cargo.txt'),
        'toolchain': read_file('$OUTDIR/rustup.txt'),
    },
    'os': {
        'name': read_file('$OUTDIR/os.txt'),
        'kernel': read_file('$OUTDIR/kernel.txt'),
    },
    'hardware': {
        'cpu': read_file('$OUTDIR/cpu.txt'),
        'cores': read_file('$OUTDIR/cpu_cores.txt'),
        'memory': read_file('$OUTDIR/memory.txt'),
        'hostname': read_file('$OUTDIR/hostname.txt'),
    },
    'dependencies': read_file('$OUTDIR/dependencies-compact.txt'),
}
with open('$OUTDIR/metadata.json', 'w') as f:
    json.dump(d, f, indent=2)
print('metadata.json written')
"

echo "Environment captured to $OUTDIR"
echo "  Files: $(ls "$OUTDIR" | wc -l)"
