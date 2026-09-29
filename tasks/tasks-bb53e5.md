---
id: tasks-bb53e5
title: A note written to a task from the main checkout while a worktree holds a modified copy of the same record diverges silently
status: idea
priority: 2
created: 2026-09-24T14:50:14Z
updated: 2026-09-29T20:50:25Z
depends: []
parent: tasks-c4ad8e
tags: [feedback, gap, "from:ai"]
agent: "claude-code/claude-opus-5-5[1m]"
---

tasks note <id> run in the main checkout after git worktree add: the write lands in the main checkout's copy while the worktree's copy (where the task's gate notes and done will land) is already modified; no warning. Expected: a warning naming the other worktree with a modified copy of the record, or an opt-in to route writes to the worktree that holds it.

## Notes

- 2026-09-25T12:27:49Z (main): Hit for real 2026-09-25 on tasks-142d2f: note/park/start written in the main checkout after git worktree add diverged from the worktree copy; the only signal was a later write's 'newer than this copy' warning. Reconciled by replaying the notes in the worktree and discarding the main copy.
- 2026-09-29T20:50:25Z (main): scope: briefed; the equal-stamp case needs a home checkout (claim names main when start precedes worktree add, per tasks-142d2f), which design tasks-ab8d2d settles; parented to tasks-c4ad8e; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
