---
id: tasks-ccbdf2
title: Persist and expose the process contract
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: feat/tasks-61cc5c-process
created: 2026-09-13T17:33:23Z
updated: 2026-09-13T17:52:35Z
started: 2026-09-13T17:47:15Z
completed: 2026-09-13T17:52:35Z
depends: []
parent: tasks-61cc5c
tags: []
spec: docs/specs/2026-09-13-task-process-design.md
plan: docs/plans/2026-09-13-task-process.md
step: "Task 1: Persist and expose the process contract"
---

Implement Task 1 of docs/plans/2026-09-13-task-process.md after the implementation plan is approved. Add the optional direct/planned process field through the record, CLI, output and doing-only warning paths; verify with the existing tests, gate, and installed binary. Intended process: direct, because the reviewed plan supplies the decisions; set --process direct explicitly once the supporting binary exists. Parent intended process: planned. No readiness or claim-store changes.

## Notes

- 2026-09-13T17:33:46Z (feat/tasks-61cc5c-process): parked (waiting on user, review): Review .worktrees/tasks-61cc5c-process/docs/plans/2026-09-13-task-process.md before implementing Task 1
- 2026-09-13T17:50:48Z (feat/tasks-61cc5c-process): Plan approved for Task 1 execution; initial CLI tests failed on unknown --process and missing doing warning, and record test failed on unknown process key. Focused tests now pass; full gate and code review in progress.
- 2026-09-13T17:52:35Z (feat/tasks-61cc5c-process): Task 1 verified: focused tests red then green; just gate passed 171 unit and 289 CLI tests; independent review found no material issues; cargo install succeeded. Installed binary set parent planned and both children direct. tasks check has no errors and only process_missing on unrelated doing task tasks-7ba741; no backfill performed.
- 2026-09-13T17:52:35Z (feat/tasks-61cc5c-process): Added explicit optional process field, add/edit/clear and completion, JSON/pretty projections, doing-only warning, and regression coverage; verified and installed.
