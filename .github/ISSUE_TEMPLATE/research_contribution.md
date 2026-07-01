---
name: Research Contribution
about: Submit a new experimental result, finding, or research artifact
title: "[RESEARCH] "
labels: research
assignees: ''
---

## Hypothesis

What question were you trying to answer? State your hypothesis clearly and, if possible, in a falsifiable form.

## Methodology

Describe how you tested the hypothesis:

- **Experiment design:** what configurations, parameters, and controls
- **Metrics collected:** what you measured and how
- **Statistical methods:** trials, confidence intervals, significance tests (if any)
- **Tools used:** simulator flags, custom scripts, analysis notebooks

## Expected Outcome

What did you expect to happen based on the existing literature, theory, or prior results?

## Actual Outcome

What actually happened? Be honest about negative, null, or inconclusive results.

## Environment / Configuration

```toml
# Paste your experiment configuration (e.g., experiment.toml)
node_count = 50
duration_secs = 300
seed = 42
# ...
```

- **Commit hash:**
- **Rust version:**
- **OS:**
- **Hardware:** [CPU, RAM, network conditions]

## How to Reproduce

Provide exact commands:

```bash
git checkout <commit-hash>
cargo run --release -- --paper-mode --seed <seed> --experiment <name>
```

Include any post-processing or analysis scripts.

## Relationship to Existing Results

How does this finding relate to previously documented results in this repo or the broader literature?

- Contradicts existing claim: [explain]
- Supports existing claim: [explain]
- Novel finding: [explain]

## Raw Data

Attach or link to raw CSV/JSON output. Do not submit summary statistics alone — raw data allows others to verify and extend your work.

## Additional Notes

Anything else that might help others understand, reproduce, or build on this result.
