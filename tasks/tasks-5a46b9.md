---
id: tasks-5a46b9
title: Starting a freshly added task in a worktree means hand-moving its untracked record file into the worktree
status: idea
priority: 2
created: 2026-09-14T14:25:49Z
updated: 2026-09-14T14:25:49Z
depends: []
tags: [feedback, friction, "from:ai"]
agent: "claude-code/claude-opus-5[1m]"
---

add from the main checkout, then git worktree add for the work: the task's .md is untracked in the main checkout and absent from the worktree, so done-in-the-same-commit needs a manual mv. Expected: start (or add) to notice the worktree, or a documented step.
