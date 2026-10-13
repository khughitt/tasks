---
id: tasks-7b511a
title: "A reviewed implementation plan instructs tasks list --plan, but list rejects that flag; task children must be found with --parent."
status: idea
priority: 2
reviewed: true
created: 2026-10-09T04:58:48Z
updated: 2026-10-13T10:49:15Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
---

## Notes

- 2026-10-13T10:49:15Z (main): scope: drop; list --plan landed in 3205f58 (2026-10-11, after this report); verified: list --plan docs/plans/2026-10-09-task-move.md --status done returns the plan's 10 steps. It matches the stored reference exactly, so the task-move plan's own 'list --plan task-move' still returns []; that plan is complete; proposal: drop as landed in 3205f58
