---
id: tasks-d37cf5
title: Adopt and document the process policy
status: done
priority: 2
size: s
complexity: low
process: direct
owner: feat/tasks-61cc5c-process
created: 2026-09-13T17:33:45Z
updated: 2026-09-13T18:11:55Z
started: 2026-09-13T18:06:07Z
completed: 2026-09-13T18:11:55Z
depends: [tasks-ccbdf2]
parent: tasks-61cc5c
tags: []
spec: docs/specs/2026-09-13-task-process-design.md
plan: docs/plans/2026-09-13-task-process.md
step: "Task 2: Adopt and document the process policy"
---

Implement Task 2 of docs/plans/2026-09-13-task-process.md after plan approval and tasks-ccbdf2. Update the shipped tasks/scope/curate skills, repo instruction policy, README and base reference; preserve document review gates and doing-only warnings. Intended process: direct; set the explicit field using the binary installed by Task 1. Verify the three policy scenarios and just check; report ai-69ccac and ai-e8dcc5 as separate rollout follow-ups, not completed by this repo.

## Notes

- 2026-09-13T17:34:16Z (feat/tasks-61cc5c-process): parked (waiting on user, review): Review .worktrees/tasks-61cc5c-process/docs/plans/2026-09-13-task-process.md; then wait for tasks-ccbdf2 before Task 2
- 2026-09-13T17:52:35Z (feat/tasks-61cc5c-process): parked (waiting on agent, session): Plan approved; next run tasks start tasks-d37cf5 and execute Task 2 in .worktrees/tasks-61cc5c-process/docs/plans/2026-09-13-task-process.md; Task 1 is complete and process is already direct
- 2026-09-13T18:11:34Z (feat/tasks-61cc5c-process): Updated repo adoption so process outranks generic brainstorming triggers; documented explicit --complexity <level> --process <value> together on plan children, scoped/curated assignment, doing-only warning and separate ai rollout. All three skills validated; manual three-scenario review found no material gaps; just check passed with only the expected tasks-7ba741 warning. Left that record unchanged for main-checkout merge handling.
- 2026-09-13T18:11:55Z (feat/tasks-61cc5c-process): Adopted process-driven brainstorming and worktree policy in repo instructions, aligned all three skills and public docs, and verified the manual scenarios and checks; ai rollout remains separate.
