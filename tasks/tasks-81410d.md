---
id: tasks-81410d
title: "needs-review queue lists an idea as unreviewed on the main checkout while an unmerged worktree branch has already promoted it to todo with children; scope cannot see branch-side state, and any audit note written on main would conflict with the branch's edits to the same record"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-22T22:30:23Z
created: 2026-10-20T23:59:17Z
updated: 2026-10-22T22:30:23Z
depends: []
tags: [feedback, gap, "from:material"]
---

## Notes

- 2026-10-22T22:30:23Z (main): scope: drop; duplicate of tasks-f3910b, whose scoped acceptance criteria cover show warnings for newer sibling records and scope skips on that warning or stale_copy; checked against c6e144a (reads stay local; writes compare worktrees); brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop tasks-81410d as covered by tasks-f3910b
