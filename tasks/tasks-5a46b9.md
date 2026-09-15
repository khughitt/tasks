---
id: tasks-5a46b9
title: Commit a new task record before creating its worktree
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: 63742b-bootstrap
created: 2026-09-14T14:25:49Z
updated: 2026-09-15T21:07:23Z
started: 2026-09-15T21:05:57Z
completed: 2026-09-15T21:07:23Z
depends: []
parent: tasks-02769c
tags: [feedback, friction, "from:ai"]
agent: "claude-code/claude-opus-5[1m]"
---

Why: a worktree starts from committed files, so an uncommitted task record cannot be completed there without divergence.\nDone: the task workflow requires committing the task record before git worktree add.\nWhere to look: AGENTS.md and skills/tasks/SKILL.md.

## Notes

- 2026-09-15T21:02:02Z (main): scope: briefed; grouped under tasks-02769c pending the bootstrap-contract research; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
- 2026-09-15T21:05:12Z (63742b-bootstrap): finding: tasks-63742b recommends committing the task record before git worktree add; the CLI must not copy it into another checkout.
- 2026-09-15T21:05:57Z (63742b-bootstrap): scope: scoped; direct documentation change established by tasks-63742b; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
- 2026-09-15T21:07:23Z (63742b-bootstrap): Documented committing task records before creating a worktree.
