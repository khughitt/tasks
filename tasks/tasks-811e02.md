---
id: tasks-811e02
title: "Quiet queue: park work that needs an idle host and list it across projects before bed"
status: todo
priority: 2
size: m
complexity: high
created: 2026-09-13T14:01:44Z
updated: 2026-09-13T14:04:25Z
depends: []
tags: [cli, picker]
spec: docs/specs/2026-09-13-quiet-queue-design.md
---

Benchmarks and captures that need a host free of competing load (GPU captures in material, sweep benchmarks in atoms) stall under park --reason environment, mixed with checkout and dependency parks, and nobody lists parked work across projects at bedtime. Add a distinct park reason (quiet) for prepared, unattended work that only needs an idle machine, and a queue view across all projects that prints the top item as a resume brief: worktree, next step, expected duration, host condition (quiet desktop vs no desktop session). Attended sessions stay under decision. Idle detection and auto-launch are a later phase.
