---
id: tasks-d6bd82
title: "tasks -C on a checkout directory that no longer exists fails with the generic io error kind, so a caller cannot tell a removed checkout from a real read failure without inspecting the filesystem itself"
status: idea
priority: 2
created: 2026-10-21T12:23:32Z
updated: 2026-10-22T23:11:11Z
depends: []
parent: tasks-a7460c
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
---

Seen as {"error":{"detail":"io: No such file or directory (os error 2)","kind":"io"}} for show and list; a half-removed worktree (directory present, .git gone) gives kind io with a git rev-parse exit 128 detail. A typed kind (for example checkout_missing) for both would let indexers treat a removed worktree as unlisted without their own lstat race.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-missing-checkout-errors-brief.md — evidence, constraints and open questions
