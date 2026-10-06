---
id: tasks-6b2867
title: "A park recorded from a worktree survives the task's close from another checkout, and no command clears it: prime keeps listing the closed task as parked"
status: idea
priority: 2
created: 2026-10-06T09:08:25Z
updated: 2026-10-06T09:45:49Z
depends: []
parent: tasks-984769
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

Task closed with done in the main checkout while its park was keyed to a since-removed worktree; prime shows it under parked, list --parked does not; no unpark command exists, so only hand-editing the state store would clear it.

## Notes

- 2026-10-06T09:45:48Z (main): scope: briefed; grouped record visibility and recovery evidence under tasks-984769; bounded reproductions precede a routing or recovery change; brief: docs/notes/2026-10-06-record-continuity-brief.md
