---
id: tasks-5a46b9
title: Starting a freshly added task in a worktree means hand-moving its untracked record file into the worktree
status: idea
priority: 2
created: 2026-09-14T14:25:49Z
updated: 2026-09-15T21:02:02Z
depends: []
parent: tasks-02769c
tags: [feedback, friction, "from:ai"]
agent: "claude-code/claude-opus-5[1m]"
---

add from the main checkout, then git worktree add for the work: the task's .md is untracked in the main checkout and absent from the worktree, so done-in-the-same-commit needs a manual mv. Expected: start (or add) to notice the worktree, or a documented step.

## Notes

- 2026-09-15T21:02:02Z (main): scope: briefed; grouped under tasks-02769c pending the bootstrap-contract research; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
