---
id: tasks-06afbe
title: list --needs-review and show report a record as an unreviewed idea while a newer copy in another worktree has already promoted it to a todo goal with children; only writes notice the newer copy (stale_copy)
status: idea
priority: 2
created: 2026-10-18T23:02:56Z
updated: 2026-10-18T23:02:56Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

Seen during a scope pass: the queue offered an idea whose branch copy (unmerged worktree) was already a todo implementation goal with plan children; show from the main checkout gave no warning. Expected: the queue row or show to flag that a newer copy exists in another checkout (as stale_copy does on write), so a scope pass skips it without manual worktree diffing.
