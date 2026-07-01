---
name: Good First Issue
about: A well-scoped entry point for new contributors
title: "[good-first-issue] "
labels: good first issue
assignees: ''
---

## Description

Briefly describe the task and why it's a good starting point.

## Estimated Difficulty

- [ ] **Easy** — minimal codebase exploration, straightforward change
- [ ] **Medium** — moderate codebase exploration, some new understanding needed
- [ ] **Hard** — requires understanding multiple subsystems

## Estimated Time

- [ ] < 1 hour
- [ ] 1–3 hours
- [ ] 3–8 hours
- [ ] Multiple sessions

## Background Needed

What should you know before starting?
- Rust fundamentals (ownership, traits, enums)
- Understanding of the relevant module (link to source)

## Where to Start in the Codebase

Point to the relevant files, functions, or modules:

- **Main file:** `src/<module>/<file>.rs`
- **Key functions/types:** `fn do_thing()`, `struct ImportantType`
- **Related tests:** `tests/<test_file>.rs`

## Acceptance Criteria

- [ ] What should the code do?
- [ ] What tests should pass?
- [ ] What documentation should be updated?

## Testing Instructions

```bash
cargo test           # all tests must pass
cargo clippy -D warnings  # zero warnings policy
```

If the change affects benchmarks:
```bash
cargo bench          # no regressions > 5%
```

## Relevant Roadmap Item

Which roadmap direction does this contribute to? (See [ROADMAP.md](https://github.com/cianmag/neuron-wire/blob/master/ROADMAP.md).)

## How to Ask for Help

- Open a comment on this issue with your approach and questions.
- Join the [Discord](https://discord.gg/neuron-wire) and mention this issue in the #contributors channel.
- Tag `@cianmag` in the issue for maintainer attention.
