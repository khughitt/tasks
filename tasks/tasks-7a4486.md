---
id: tasks-7a4486
title: Cross-project dependencies cannot resolve new records held only in another worktree
status: idea
priority: 2
created: 2026-10-23T00:20:47Z
updated: 2026-10-23T00:20:47Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: codex
---

Creating a task with a cross-project dependency fails with unresolvable_id when the dependency record is committed only in the other project’s worktree. The workaround is to keep the receiving task explicitly blocked until the source records integrate, then add the dependency and unblock. Parent-task ownership does not make those child records discoverable for this lookup.
