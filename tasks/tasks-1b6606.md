---
id: tasks-1b6606
title: "the scope queue (list --needs-review) on the main checkout lists an idea as unreviewed when its record has already advanced on an unmerged worktree branch (promoted to todo, children doing there); nothing in list/show signals the divergence, so a scope pass would write conflicting edits on main"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-22T22:30:20Z
created: 2026-10-20T22:44:11Z
updated: 2026-10-22T22:30:20Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code
---

## Notes

- 2026-10-22T22:30:20Z (main): scope: drop; duplicate of tasks-f3910b, whose scoped acceptance criteria cover show warnings for newer sibling records and scope skips on that warning or stale_copy; checked against c6e144a (reads stay local; writes compare worktrees); brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop tasks-1b6606 as covered by tasks-f3910b
