---
id: tasks-1b6606
title: "the scope queue (list --needs-review) on the main checkout lists an idea as unreviewed when its record has already advanced on an unmerged worktree branch (promoted to todo, children doing there); nothing in list/show signals the divergence, so a scope pass would write conflicting edits on main"
status: dropped
priority: 2
created: 2026-10-20T22:44:11Z
updated: 2026-10-22T22:32:35Z
closed: 2026-10-22T22:32:35Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code
---

## Notes

- 2026-10-22T22:30:20Z (main): scope: drop; duplicate of tasks-f3910b, whose scoped acceptance criteria cover show warnings for newer sibling records and scope skips on that warning or stale_copy; checked against c6e144a (reads stay local; writes compare worktrees); brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop tasks-1b6606 as covered by tasks-f3910b
- 2026-10-22T22:32:35Z (main): dropped
  provenance: {"harness_session":"codex:01a165b9-d413-7720-b812-9d25c569dc65","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-22T22:32:35Z (main): User approved duplicate drop; covered by the scoped task tasks-f3910b.
  provenance: {"harness_session":"codex:01a165b9-d413-7720-b812-9d25c569dc65","harness_session_source":"CODEX_THREAD_ID"}
