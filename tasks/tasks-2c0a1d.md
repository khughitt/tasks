---
id: tasks-2c0a1d
title: "note on a task whose newer copy lives in another worktree writes to the older copy first and warns afterwards; refusing, or naming the worktree to run from, would keep the two copies from diverging"
status: idea
priority: 2
created: 2026-09-29T11:36:20Z
updated: 2026-09-29T11:36:20Z
depends: []
tags: [feedback, friction, "from:ns"]
agent: claude-code/claude-fable-5-1
---

Command: tasks note <id> "..." run from the main checkout while the record had been written from a task worktree. Result: the note landed in the main checkout's copy with the warning 'is newer than this copy ... this write may omit changes from that copy'. Expected: no write, or a write to the newer copy.
