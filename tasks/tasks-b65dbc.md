---
id: tasks-b65dbc
title: Establish whether need filters cover unprepared quiet-host work
status: todo
priority: 3
size: s
complexity: mid
process: direct
created: 2026-10-08T10:53:55Z
updated: 2026-10-13T09:21:39Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-00bac8
agent: codex
---

Question: Do existing list/ready --need filters adequately expose unprepared quiet-host tasks, or is a distinct discovery view needed beside the prepared quiet queue?
Where to start: docs/notes/2026-09-30-work-selection-brief.md; tasks-00bac8; src/commands/quiet.rs, src/commands/parked.rs::rows_preferring, src/commands/list.rs, src/filter.rs; docs/specs/2026-09-13-quiet-queue-design.md sections 4-5 and docs/specs/2026-10-03-lanes-needs-groups-design.md sections 4 and 11; existing needs/quiet integration cases in tests/cli.rs.
Bound: Trace these read paths and compare a small fixture matrix: an unclaimed todo with a named quiet need; a prepared quiet park; a blocked/deferred/user-held task; a live claim or held resource; and an unrelated exclusive device need. Compare list versus ready with --need and TASKS_WITHOUT, including how an intentionally idle session selects work. Isolate both XDG_CONFIG_HOME and XDG_STATE_HOME for any scratch CLI probe. No live host use, task launch, schema change or implementation. A task need does not supply park minutes, next step or idle/headless instructions; exclusivity alone does not establish idle-host suitability.
Expected result: Record observed command outputs and recommend documented existing filters or a separately designed discovery extension. Name any unmet need and proposed selection/output contract; keep vocabulary/park-recipe unification a separate decision. Update this task and the brief. P3/s/mid/direct: bounded comparison of existing contracts with a checkable matrix; no host reservation is needed.
Ideas it wakes: On completion, run tasks note on tasks-00bac8 with the finding before this result's branch merges, with the brief update, so default scope selection reconsiders it.
