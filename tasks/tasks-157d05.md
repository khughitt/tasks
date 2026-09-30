---
id: tasks-157d05
title: Design dated resumption of parked work from the work-selection brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T10:04:57Z
updated: 2026-09-30T10:04:57Z
depends: []
parent: tasks-46d207
tags: []
agent: codex
---

Why: tasks-44b889 reports doing tasks missing their calendar revisit because deferral rejects doing and park has no date.
Where to start: docs/notes/2026-09-30-work-selection-brief.md; docs/specs/2026-09-15-defer-design.md sections 2-5; the park design; src/defer.rs, src/commands/edit.rs, src/commands/park.rs, src/commands/parked.rs, src/commands/list.rs, and src/claims.rs.
Bound: compare allowing defer on parked doing records, a date in the park store, and existing todo-plus-defer. Decide synced versus host-local persistence, early resume and re-park clearing, user-versus-agent waiting, recurrence interaction, and due visibility in prime. A due date must not silently authorize resuming user-held work. No timers, notifications, or implementation during design.
Done: a reviewed design specifies before/due/after examples and rejection/clearing rules across pickers; obtain the implementation-plan review before code.
Ideas it wakes: record the finding on tasks-44b889 and update the brief in the same commit as the result.
