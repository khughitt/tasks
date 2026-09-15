---
id: tasks-63742b
title: Recommend a portable fresh-worktree bootstrap contract
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: 63742b-bootstrap
created: 2026-09-15T21:01:47Z
updated: 2026-09-15T21:05:12Z
started: 2026-09-15T21:02:39Z
completed: 2026-09-15T21:05:12Z
depends: []
parent: tasks-02769c
tags: []
agent: codex
---

Question: What project-owned declaration and non-interactive sequence can prepare a fresh worktree without the task CLI copying records or executing arbitrary commands?
Where to start: docs/notes/2026-09-15-fresh-worktree-brief.md, skills/tasks/SKILL.md, repository agent instructions, and the setup conventions in projects cited by tasks-5a46b9 and tasks-5e6971.
Bound: Compare existing conventions and identify the smallest portable contract; do not implement a CLI/config change.
Expected result: Record the evidence and a recommended workflow on this task and in the brief.
Ideas it wakes: On completion, run tasks note on tasks-5a46b9 and tasks-5e6971 with the finding, in the same commit as this result.

## Notes

- 2026-09-15T21:05:12Z (63742b-bootstrap): Recorded the committed-record and documented-bootstrap workflow in docs/notes/2026-09-15-fresh-worktree-brief.md.
