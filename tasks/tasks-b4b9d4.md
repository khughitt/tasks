---
id: tasks-b4b9d4
title: "Retired projects: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-04T16:44:19Z
updated: 2026-10-04T16:44:20Z
depends: []
tags: []
source: ops-3b3e4d
agent: claude-code/claude-opus-5-5
---

From the tasks-132852 final review, deferred: (1) rename --adopt checks only the local config, not the new prefix's registered root; (2) retire on an already-retired project overwrites date/reason silently; (3) the stored successor is the alias as typed, not canonical_prefix(s); (4) feedback owners list and --all-projects id completion still offer retired projects; (5) group set reads retirement without the project lock; (6) check in a pre-retirement worktree does not consult the registered [retired]; (7) unreadable_checkout for a prunable worktree suggests no git worktree prune; (8) a registered root that is a plain file makes init --force fail with an unlabelled NotADirectory io error; (9) park is missing from the pre-retirement worktree write loop test; (10) date check accepts +026-10-04 and 0000-01-01.

## Notes

- 2026-10-04T16:44:19Z (main): concerns: tasks-132852 extension — review minors deferred at close
