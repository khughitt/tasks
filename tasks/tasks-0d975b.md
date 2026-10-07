---
id: tasks-0d975b
title: "tasks skill: the same-commit rule and task ids in a linked checkout"
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: feat/feedback-unregistered
created: 2026-10-07T16:48:39Z
updated: 2026-10-07T17:12:11Z
started: 2026-10-07T16:54:41Z
completed: 2026-10-07T16:55:07Z
depends: []
parent: tasks-a7460c
tags: [cross-project]
source: tack-956458
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Piece 7 (tasks-skill half) of ops docs/specs/2026-10-01-linked-checkouts-design.md §4.4, re-homed from tack-956458. The skill's linked-project paragraph (from tasks-9b0a2e) says where records live; add what changes in practice: 'tasks done in the same commit as the code' reads as the matching commit in the overlay repository; commit the record before git worktree add means the overlay commit; task ids never appear in the code repository (commits, branches, pull requests, docs cite the repository's own issue key, e.g. a Linear key via --source).

## Notes

- 2026-10-07T16:54:41Z (feat/feedback-unregistered): started
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T16:55:07Z (feat/feedback-unregistered): done
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T16:55:07Z (feat/feedback-unregistered): tasks skill: in a linked checkout the same-commit and commit-before-worktree rules read as overlay commits, the worktree is added in the code checkout, and task ids never appear in the code repository (cite its issue key via --source)
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T17:07:15Z (feat/feedback-unregistered): review: impl round 1 — verdict: accept; findings: Minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-07T17:12:11Z (feat/feedback-unregistered): review minors addressed after close: commit order (code first), <prefix>/tasks/ named, issue key only when there is one
