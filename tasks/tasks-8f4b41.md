---
id: tasks-8f4b41
title: start in the main checkout is invisible to a worktree created afterwards; the two task files then diverge and the merge conflicts
status: idea
priority: 2
created: 2026-09-05T08:32:43Z
updated: 2026-09-05T08:32:43Z
depends: []
tags: [feedback, friction, "from:material"]
---

tasks start <id> in the main checkout, then git worktree add and tasks note <id> in the worktree: the worktree's copy stays status todo with no owner, the main checkout has an uncommitted doing/owner edit, and git merge refuses until one side is discarded. Expected either start to be committed automatically or a warning that the file is uncommitted.
