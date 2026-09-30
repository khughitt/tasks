---
id: tasks-cff04e
title: "Starting a task in the main checkout and then in its new worktree within the same second leaves the worktree copy unwritable: stale_copy reports equal stamps with different content, although the worktree copy strictly extends the main one"
status: idea
priority: 2
created: 2026-09-30T14:19:22Z
updated: 2026-09-30T14:19:22Z
depends: []
tags: [feedback, friction, "from:tack"]
agent: claude-code/claude-opus-5-5
---

Followed the documented sequence (start, commit, worktree add, start in worktree) in one shell command; every later note from the worktree refused with stale_copy until the worktree copy was reset to main's, losing the resumed note.
