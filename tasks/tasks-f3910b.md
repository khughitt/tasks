---
id: tasks-f3910b
title: "scope queue (--needs-review) on the main checkout lists an idea that an unmerged branch has already promoted to a todo goal with children; the scope skill has no rule for this, and a main-side audit note would conflict with the branch's record of the task"
status: idea
priority: 2
reviewed: true
created: 2026-10-15T18:15:26Z
updated: 2026-10-17T02:28:10Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code
---

## Notes

- 2026-10-17T02:28:10Z (main): scope: briefed; reproduced in a scratch project: with the promoting worktree present, main's queue and show look clean and only the write refuses stale_copy (its retry points into the branch); with the worktree removed and branch kept, the note writes silently. Lean: show warns on a newer sibling copy + scope §6 skips on it or on stale_copy; tasks-16de1c and tasks-2b2b49 proposed as duplicates; brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md
