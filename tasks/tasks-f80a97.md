---
id: tasks-f80a97
title: Design successor task migration from the brief
status: todo
priority: 3
size: m
complexity: high
process: planned
created: 2026-10-23T11:00:07Z
updated: 2026-10-23T11:00:07Z
depends: []
parent: tasks-a7460c
tags: []
source: tasks-5ab7e8
agent: codex
---

Why: tasks-5ab7e8 requests a successor-migration triage view and one transaction carrying selected tasks with notes, parents and dependencies intact. Existing single-task move refuses parented tasks and goals, so repeating it does not satisfy the request.

Done: Produce a user-reviewed design spec and then a user-reviewed implementation plan for carry/drop/close decisions and graph-preserving migration. Settle whether one transaction requires all-or-nothing rollback or explicit forward recovery; preserve the original requirement unless the user reviews a different guarantee. Define mixed parent/child decisions, destination id mapping, external dependency readiness, attachments and documents, active claims, stale records, interruption/retry and retirement ordering. Bound the first version to one source and one registered successor; no implementation or live migration in this task. Any CLI vocabulary starts in ops before a parser change.

Where: src/commands/move_.rs, src/moving/{preflight,journal,mod}.rs, src/commands/retire.rs, src/tombstone.rs, tests/move.rs and the implemented task-move design. Check the historical incident and related project-generations idea when evidence outside this scoping checkout is available; their graph and policy details have not been verified here.

Ideas it wakes: On completion, update the migration brief and run `tasks note tasks-5ab7e8 --no-reviewed "<design finding and review outcome>"` before this result's branch merges.

## References

- move-design (input, required, kind=spec): docs/specs/2026-10-08-task-move-design.md — implemented single-task guarantees and deliberate subtree exclusion; verify against current code
