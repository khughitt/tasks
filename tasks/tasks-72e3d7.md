---
id: tasks-72e3d7
title: Settle detached-record migration recovery
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-24T11:15:05Z
updated: 2026-10-24T11:15:05Z
depends: []
parent: tasks-a7460c
tags: []
source: docs/notes/2026-10-24-detached-migration-recovery-brief.md
agent: codex
---

Make detached-record migrations recoverable without losing checkout identity, configuration changes or unrelated staged work. First milestone: establish supported relocation, discovery during the unpaired window, and configuration-commit behavior from the three reports.

Why: [ref:brief] traces tasks-e9ce63, tasks-13d935 and tasks-4e656c to existing guards and conflicting documentation. Their outcomes require an explicit recommendation before a fix is scoped.
Done: the research records a disposition for every ask, wakes each waiting idea, and each member then receives an implementation scope or another explicit disposition. This goal approves no production change.
Where: start with [ref:brief], the approved records-home and record-commit designs, and the named init/discovery code.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-detached-migration-recovery-brief.md — incident evidence, contract tensions and research bounds
