#!/usr/bin/env bash
# reproduce.sh — One-command reproduction of every figure in the neuron-wire paper series.
#
# Usage:
#   bash scripts/reproduce.sh              # Run all experiments
#   bash scripts/reproduce.sh --figures 1  # Run only Figure 1
#   bash scripts/reproduce.sh --figures 1-3  # Run Figures 1 through 3
#   bash scripts/reproduce.sh --validate    # Validate against known-good only
#   bash scripts/reproduce.sh --plots-only  # Regenerate plots from existing CSVs
#
# Output:
#   results/            — Per-figure directories with CSVs, configs, logs, plots
#   results/metadata/   — Full environment capture (compiler, OS, deps, hashes)
#   results/figures/    — Publication-ready PNG plots
#
# Exit code: 0 = all figures reproduced and validated, 1 = any failure

set -euo pipefail
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$0")/..")"

REPO_ROOT="$(pwd)"
RESULTS_DIR="$REPO_ROOT/results"
EXPERIMENTS="$REPO_ROOT/experiments"
KNOWN_GOOD="$REPO_ROOT/known-good"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

# ── Colours ──────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}✅${NC} $1"; }
fail() { echo -e "  ${RED}❌${NC} $1"; }
info() { echo -e "  ${CYAN}ℹ${NC}  $1"; }
warn() { echo -e "  ${YELLOW}⚠${NC}  $1"; }
heading() { echo -e "\n${CYAN}═══ $1 ═══${NC}"; }

# ── Parse args ──────────────────────────────────────────────────────────
FIGURES="${1:-all}"
VALIDATE_ONLY=false
PLOTS_ONLY=false
for arg in "$@"; do
  case "$arg" in
    --validate) VALIDATE_ONLY=true ;;
    --plots-only) PLOTS_ONLY=true ;;
  esac
done

# ── Step 0: Check prerequisites ─────────────────────────────────────────
heading "Prerequisites"
command -v cargo &>/dev/null || { fail "Rust not found"; exit 1; }
command -v python3 &>/dev/null || { fail "Python 3 not found"; exit 1; }
command -v git &>/dev/null || { fail "git not found"; exit 1; }
rustc --version | head -1
cargo --version | head -1
git --version
info "Repository: $(git rev-parse --short HEAD) on $(git rev-parse --abbrev-ref HEAD)"
pass "All prerequisites met"

mkdir -p "$RESULTS_DIR/env"
mkdir -p "$RESULTS_DIR/figures"

# ── Step 1: Capture environment ────────────────────────────────────────
if [ "$PLOTS_ONLY" = false ] && [ "$VALIDATE_ONLY" = false ]; then
  heading "Capturing Environment"
  bash "$REPO_ROOT/scripts/capture_env.sh" "$RESULTS_DIR/env/$TIMESTAMP"
  pass "Environment captured → $RESULTS_DIR/env/$TIMESTAMP"
fi

# ── Step 2: Build ───────────────────────────────────────────────────────
if [ "$PLOTS_ONLY" = false ] && [ "$VALIDATE_ONLY" = false ]; then
  heading "Building neuron-wire (release)"
  cargo build --release 2>&1 | tail -1
  pass "Build complete"
fi

# ── Step 3: Run experiments ─────────────────────────────────────────────
if [ "$VALIDATE_ONLY" = false ]; then
  heading "Running Experiments"

  # Determine which experiment files to run
  if [ "$FIGURES" = "all" ]; then
    CONFIG_FILES=("$EXPERIMENTS"/figure-*.toml)
  elif [[ "$FIGURES" =~ ^[0-9]+$ ]]; then
    CONFIG_FILES=("$EXPERIMENTS"/figure-${FIGURES}-*.toml)
  elif [[ "$FIGURES" =~ ^([0-9]+)-([0-9]+)$ ]]; then
    CONFIG_FILES=()
    for i in $(seq "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"); do
      CONFIG_FILES+=("$EXPERIMENTS"/figure-${i}-*.toml)
    done
  else
    CONFIG_FILES=("$FIGURES")
  fi

  for config in "${CONFIG_FILES[@]}"; do
    [ -f "$config" ] || { warn "Config not found: $config"; continue; }
    figure_name=$(basename "$config" .toml)
    info "Running: $figure_name"

    # Build output directory for this figure
    figure_dir="$RESULTS_DIR/$figure_name"
    mkdir -p "$figure_dir"

    # Run simulation with this config
    if cargo run --release --example simulate -- \
      --config "$config" \
      --output-dir "$figure_dir" 2>&1 | tail -5; then
      pass "$figure_name completed"
    else
      fail "$figure_name failed"
    fi
  done
fi

# ── Step 4: Validate against known-good ─────────────────────────────────
heading "Validation"
if [ -d "$KNOWN_GOOD" ]; then
  python3 "$REPO_ROOT/scripts/validate_repro.py" "$RESULTS_DIR" --known-good "$KNOWN_GOOD" && {
    pass "All results validated against known-good"
  } || {
    fail "Results differ from known-good — see diff above"
    exit 1
  }
else
  warn "No known-good directory found. Skipping validation."
  info "To create known-good: cp -r results/figure-* known-good/"
fi

# ── Step 5: Generate figures ────────────────────────────────────────────
heading "Generating Figures"
if command -v python3 &>/dev/null && [ -f "$REPO_ROOT/scripts/requirements.txt" ]; then
  # Check if matplotlib is available
  if python3 -c "import matplotlib" 2>/dev/null; then
    python3 "$REPO_ROOT/scripts/generate_figures.py" \
      --input-dir "$RESULTS_DIR" \
      --output-dir "$RESULTS_DIR/figures" \
      --format png
    pass "Figures generated → $RESULTS_DIR/figures/"
  else
    warn "matplotlib not installed. Install with: pip install -r scripts/requirements.txt"
    info "Figure generation requires: matplotlib, numpy, scipy"
  fi
else
  warn "Python requirements not found. Skipping figure generation."
fi

# ── Summary ──────────────────────────────────────────────────────────────
heading "Summary"
echo ""
echo "  Results:     $RESULTS_DIR/"
echo "  Figures:     $RESULTS_DIR/figures/"
echo "  Environment: $RESULTS_DIR/env/$TIMESTAMP/"
echo "  Commit:      $(git rev-parse --short HEAD)"
echo ""
echo "  To regenerate all figures from existing data:"
echo "    bash scripts/reproduce.sh --plots-only"
echo ""
echo "  To validate against known-good:"
echo "    bash scripts/reproduce.sh --validate"
echo ""
pass "Reproduction complete"

# ── Generate quick-reference summary table ──────────────────────────────
if [ -d "$RESULTS_DIR" ]; then
  summary_file="$RESULTS_DIR/reproduction-summary-$TIMESTAMP.csv"
  echo "figure,status,config,outputs" > "$summary_file"
  for d in "$RESULTS_DIR"/figure-*/; do
    figure_name=$(basename "$d")
    if [ -f "$d/summary.csv" ]; then
      status="PASS"
      outputs=$(ls "$d"/*.csv 2>/dev/null | wc -l)
    else
      status="NO_DATA"
      outputs="0"
    fi
    echo "$figure_name,$status,$d/*.toml,$outputs CSV files" >> "$summary_file"
  done
  info "Summary table → $summary_file"
fi
