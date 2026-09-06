---
id: tasks-cf1bda
title: Mark tasks that are good candidates for parallel work
status: todo
priority: 2
size: m
created: 2026-09-06T00:32:54Z
updated: 2026-09-06T09:34:52Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-06-parallel-candidates-design.md
plan: docs/plans/2026-09-06-parallel-candidates.md
---

An option to flag a task as safe to work on in parallel with others (no shared files or state with the rest of the ready list), so ready and next can surface parallel candidates and a session that dispatches several agents knows which to hand out. Open questions: a tag versus a field, and whether the flag is set by hand or inferred from disjoint spec/plan references.

## Notes

- 2026-09-06T09:21:27Z (parallel-candidates): design landed 2026-09-06: per-task boolean field (not tag, not lanes), hand-set with no inference; ready --parallel filter, conditional || column in pretty tables; is_ready/ready_order and next unchanged
