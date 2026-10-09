---
id: tasks-9de6aa
title: "start warns 'commit it before branching' in the task's own worktree, where the protocol's start just rewrote the committed record"
status: doing
priority: 3
size: s
complexity: low
process: direct
owner: main
created: 2026-10-02T19:18:21Z
updated: 2026-10-09T13:38:36Z
started: 2026-10-09T13:38:30Z
depends: []
parent: tasks-0e432f
tags: [cli]
agent: claude-code/claude-opus-5-5
---

Why: the documented sequence commits the record, runs git worktree add, then tasks start in the worktree. That start appends 'resumed', so the record is uncommitted there and warn_if_uncommitted_with_worktrees (src/commands/status.rs) always says to commit it before branching, which has already happened. Agents learn to ignore the warning, which then fails to flag the real case.

Done: start stays silent when it runs in a linked worktree whose HEAD already contains the record (the branch was made after the commit); the warning still fires in the main checkout when the record is uncommitted and other worktrees exist. Cover both in tests/cli.rs and keep the JSON shape.

## Notes

- 2026-10-09T13:38:30Z (main): started
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T13:38:36Z (main): Same friction reported as tasks-c74a8b and tasks-81d277 (start after worktree creation and on resume); this fix covers both
