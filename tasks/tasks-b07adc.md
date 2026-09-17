---
id: tasks-b07adc
title: Stamp lifecycle notes without changing claims
status: todo
priority: 2
size: m
complexity: high
process: direct
created: 2026-09-17T21:08:28Z
updated: 2026-09-17T21:48:52Z
depends: [tasks-d51eda]
parent: tasks-c9199a
tags: []
source: tasks-c9199a
agent: codex
plan: docs/plans/2026-09-17-lifecycle-provenance.md
step: "Task 2: Stamp lifecycle notes without changing claims"
---

Wire the approved provenance pair into shared start/resume/close transitions and existing park/close-message notes; preserve claim identity, liveness, write ordering and retries. Includes full regression checks, independent diff review, integration and installed-reader verification. This is the complete note-stamping prerequisite consumed by obs-09cb62; it does not depend on relay ancestry. Plan approved; deployment is gated on reader installation and verification on titan and europa (Europa).

## Notes

- 2026-09-17T21:42:13Z (feat/lifecycle-provenance): parked (waiting on user, environment): Before writer work: Task 1 must be merged and cargo installed on every host reading synced task files; record host/revision and isolated stamped-fixture read evidence. User must confirm hosts not inspected here. Plan approved; this is a deployment gate, not another review.
- 2026-09-17T21:48:01Z (feat/lifecycle-provenance): User confirmed the reader rollout inventory: titan and europa (Europa). Task 2 remains parked until both have the reader-only revision installed and an isolated stamped-fixture read verified. This session handles titan; Europa installation/verification requires user confirmation.
