---
id: tasks-b4b9d4
title: "Retired projects: deferred review minors"
status: doing
priority: 3
size: s
complexity: low
process: direct
owner: fix/retired-minors
created: 2026-10-04T16:44:19Z
updated: 2026-10-18T10:38:13Z
started: 2026-10-18T10:10:52Z
depends: []
parent: tasks-a7460c
tags: []
source: ops-3b3e4d
agent: claude-code/claude-opus-5-5
---

From the tasks-132852 final review, deferred: (1) rename --adopt checks only the local config, not the new prefix's registered root; (2) retire on an already-retired project overwrites date/reason silently; (3) the stored successor is the alias as typed, not canonical_prefix(s); (4) feedback owners list and --all-projects id completion still offer retired projects; (5) group set reads retirement without the project lock; (6) check in a pre-retirement worktree does not consult the registered [retired]; (7) unreadable_checkout for a prunable worktree suggests no git worktree prune; (8) a registered root that is a plain file makes init --force fail with an unlabelled NotADirectory io error; (9) park is missing from the pre-retirement worktree write loop test; (10) date check accepts +026-10-04 and 0000-01-01.

## Notes

- 2026-10-04T16:44:19Z (main): concerns: tasks-132852 extension — review minors deferred at close
- 2026-10-18T10:10:52Z (main): started
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T10:10:53Z (fix/retired-minors): resumed
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T10:27:10Z (fix/retired-minors): review: impl round 1 — verdict: revise; findings: Critical 0, Important 2, Minor 7; reviewer: claude-code
- 2026-10-18T10:38:13Z (fix/retired-minors): review: impl round 2 — verdict: accept; findings: Critical 0, Important 0, Minor 6; reviewer: claude-code
