---
id: tasks-6a0050
title: Release a recurring task's park once its recorded worktree is gone
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-09T13:50:09Z
updated: 2026-10-09T13:50:09Z
depends: []
parent: tasks-984769
tags: [cli]
source: tasks-b9b10e
agent: claude-code/claude-opus-5-5
---

Why: tasks-b9b10e's probe (attached there) parked a recurring task from a worktree under host A, closed it under host B, then removed the worktree. On A, `tasks done <id>` from the main checkout refuses with validation "already done here but has work in <wt>; finish that occurrence in its owning checkout", before and after removal, and `park` refuses invalid_transition. Once the worktree is gone that checkout no longer exists, so no command clears the park; the only path, `start` then `done`, records a spurious occurrence (new completed, "completed; next due" note, model cleared). Inside the worktree, after merging main, the same retry releases with the "already completed" warning and changes only updated.

Done: in src/commands/mod.rs::transition's recurring retry guard, a claim or park whose recorded worktree no longer exists on this host (claims and parks are host-local) no longer blocks; the retry releases it with the existing warning, records no occurrence, and changes only updated. The guard still refuses while the recorded worktree exists, and its message also says to integrate the closed record there first. Cover both in tests/cli.rs with two XDG_STATE_HOME stores, asserting record bytes and store state. JSON shapes unchanged.
