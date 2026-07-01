---
name: Bug Report
about: Report a bug to help improve neuron-wire
title: "[BUG] "
labels: bug
assignees: ''
---

## System Information

- **OS:** [e.g. Linux 6.8, macOS 14.5, Windows 11]
- **Rust Version:** [output of `rustc --version`]
- **Neuron-wire Version:** [e.g. v0.3.0, commit hash if built from source]
- **Cargo Profile:** [debug / release]

## Expected Behavior

What did you expect to happen?

## Actual Behavior

What actually happened? Include error messages, panics, or unexpected behavior.

## Reproduction Steps

Provide exact commands to reproduce the issue:

```bash
# Clone, build, and run:
git clone https://github.com/cianmag/neuron-wire
cd neuron-wire
git checkout <affected-version>
# ... exact commands ...
```

## Logs / Output

```
Paste relevant logs, backtraces, or terminal output here.
Include full backtraces (`RUST_BACKTRACE=1` or `RUST_BACKTRACE=full`).
```

## Research Context

What hypothesis, experiment, or configuration were you running when this occurred?

- **Node count:**
- **Configuration flags:** [e.g. `--paper-mode`, `--seed 42`]
- **Topology / network conditions:**
- **Experiment name or purpose:**

## Additional Context

- Does this reproduce consistently? If not, estimate frequency.
- Does it reproduce in both debug and release mode?
- Related issue / PR number (if any):
