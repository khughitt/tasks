---
id: tasks-8f4b41
title: start in the main checkout is invisible to a worktree created afterwards; the two task files then diverge and the merge conflicts
status: todo
priority: 2
size: s
created: 2026-09-05T08:32:43Z
updated: 2026-09-05T09:53:24Z
depends: []
tags: [feedback, friction, "from:material"]
spec: docs/specs/2026-09-05-work-claims-design.md
---

tasks start <id> in the main checkout, then git worktree add and tasks note <id> in the worktree: the worktree's copy stays status todo with no owner, the main checkout has an uncommitted doing/owner edit, and git merge refuses until one side is discarded. Expected either start to be committed automatically or a warning that the file is uncommitted.

## Notes

- 2026-09-05T09:53:24Z (feat/doing-claims): design: claims restore visibility; the divergent-copy warning is this task's deliverable, the merge conflict itself is a recorded gap
