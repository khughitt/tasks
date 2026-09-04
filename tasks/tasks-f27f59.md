---
id: tasks-f27f59
title: Test + CI iteration cost audit across all projects
status: dropped
priority: 2
created: 2026-09-04T12:48:03Z
updated: 2026-09-04T21:45:03Z
depends: []
tags: [testing, multi-project]
---

First hub goal once multi-project support lands. Per active project: measure full-suite wall time and how often agent full-suite runs fail; add fast/affected targets (vitest --changed, pytest-testmon / --lf, cargo-nextest) and point AGENTS.md at them for the inner loop, full suite only at commit and CI; quiet reporters to cut context cost; fix suite hygiene (sleeps, network, unshared fixtures). Consider an overlay to the Superpowers TDD skill instead of twelve AGENTS.md edits. Adaptive/stochastic test selection is deferred until this audit shows what stock tools leave on the table.

## Notes

- 2026-09-04T21:45:03Z (main): superseded by ops-65837b, the hub goal with one piece per project; this repo's piece is tasks-7ba741
