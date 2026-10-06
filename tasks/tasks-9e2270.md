---
id: tasks-9e2270
title: "tasks start warns that the record is uncommitted right after a commit, because start itself just rewrote it"
status: dropped
priority: 2
created: 2026-10-05T09:00:10Z
updated: 2026-10-06T10:39:15Z
depends: []
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Sequence: tasks add, git commit the record, tasks start in a fresh worktree -> warning 'tasks/<id>.md is uncommitted and this repo has N worktrees; commit it before branching'. The only uncommitted change is start's own owner/resumed write, so the advice cannot be followed before start and is noise after it.

## Notes

- 2026-10-06T09:45:47Z (main): scope: drop; current evidence covers this report; proposal: drop because tasks-9de6aa already scopes the same post-start uncommitted warning and its main/worktree regression checks; retain the original report; obtain a current binary/version capture if it recurs
- 2026-10-06T10:39:15Z (main): dropped
  provenance: {"harness_session":"codex:01a11097-1a86-7c90-878d-b71e466416fd","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T10:39:15Z (main): Covered by tasks-9de6aa: the same start-generated uncommitted warning is already scoped.
  provenance: {"harness_session":"codex:01a11097-1a86-7c90-878d-b71e466416fd","harness_session_source":"CODEX_SESSION_ID"}
