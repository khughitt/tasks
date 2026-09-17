---
id: tasks-b07adc
title: Stamp lifecycle notes without changing claims
status: todo
priority: 2
size: m
complexity: high
process: direct
created: 2026-09-17T21:08:28Z
updated: 2026-09-17T21:08:29Z
depends: [tasks-d51eda]
parent: tasks-c9199a
tags: []
source: tasks-c9199a
agent: codex
plan: docs/plans/2026-09-17-lifecycle-provenance.md
step: "Task 2: Stamp lifecycle notes without changing claims"
---

Wire the approved provenance pair into shared start/resume/close transitions and existing park/close-message notes; preserve claim identity, liveness, write ordering and retries. Includes full regression checks, independent diff review, integration and installed-reader verification. This is the complete note-stamping prerequisite consumed by obs-09cb62; it does not depend on relay ancestry. Wait for user approval of the plan.
