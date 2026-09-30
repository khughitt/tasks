---
id: tasks-94b9c7
title: "tasks note run from the main checkout writes an uncommitted edit to the main copy of a record whose work has moved to a worktree branch, with no warning; the edit then diverges from the worktree's copy"
status: dropped
priority: 2
created: 2026-09-30T10:00:29Z
updated: 2026-09-30T10:23:47Z
depends: []
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Flow: start and commit the record in the main checkout, create a worktree, keep working; a later tasks note issued with cwd in the main checkout lands in the main working tree while other notes land in the worktree's copy. Found only when a merge was blocked by the dirty record. Expected: a warning when another worktree of the same repository has a newer or diverged copy of the record being written, naming that worktree.

## Notes

- 2026-09-30T10:23:47Z (main): dropped
  provenance: {"harness_session":"claude-code:b0b0bafa-6168-4d1f-9d16-02ad03ecd531","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:23:47Z (main): duplicate of tasks-bb53e5 (same main-checkout note after git worktree add); evidence recorded there
  provenance: {"harness_session":"claude-code:b0b0bafa-6168-4d1f-9d16-02ad03ecd531","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
