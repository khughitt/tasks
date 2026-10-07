---
id: tasks-2d58d1
title: "feedback outside every project files with from:unregistered instead of failing no_project"
status: done
priority: 2
size: s
complexity: low
process: direct
owner: feat/feedback-unregistered
created: 2026-10-01T16:01:43Z
updated: 2026-10-07T16:54:26Z
started: 2026-10-07T16:35:45Z
completed: 2026-10-07T16:54:26Z
depends: [tasks-9b0a2e]
parent: tasks-a7460c
tags: [cross-project]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Piece 2 of ops docs/specs/2026-10-01-linked-checkouts-design.md (§4.2). Outside every project, tasks feedback tags from:unregistered; inside a linked checkout (after the linked-projects piece) it carries the linked prefix. Test both.

## Notes

- 2026-10-07T16:35:45Z (main): started
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T16:35:50Z (feat/feedback-unregistered): resumed
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T16:49:59Z (feat/feedback-unregistered): review: impl round 1 — verdict: revise; findings: Important 1, Minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-07T16:54:00Z (feat/feedback-unregistered): review: impl round 2 — verdict: accept; findings: Minor 1; reviewer: claude-code/claude-opus-5-5
- 2026-10-07T16:54:26Z (feat/feedback-unregistered): done
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T16:54:26Z (feat/feedback-unregistered): feedback outside every project files as from:unregistered (check holds the tag as written by feedback); a linked checkout reports its prefix; input validated before either project opens; README, skill and three specs amended. Two review rounds, accepted.
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
