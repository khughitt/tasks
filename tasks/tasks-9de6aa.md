---
id: tasks-9de6aa
title: "start warns 'commit it before branching' in the task's own worktree, where the protocol's start just rewrote the committed record"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-02T19:18:21Z
updated: 2026-10-02T19:18:21Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
---

Why: the documented sequence commits the record, runs git worktree add, then tasks start in the worktree. That start appends 'resumed', so the record is uncommitted there and warn_if_uncommitted_with_worktrees (src/commands/status.rs) always says to commit it before branching, which has already happened. Agents learn to ignore the warning, which then fails to flag the real case.

Done: start stays silent when it runs in a linked worktree whose HEAD already contains the record (the branch was made after the commit); the warning still fires in the main checkout when the record is uncommitted and other worktrees exist. Cover both in tests/cli.rs and keep the JSON shape.
