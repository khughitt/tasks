---
id: tasks-6b2867
title: "A park recorded from a worktree survives the task's close from another checkout, and no command clears it: prime keeps listing the closed task as parked"
status: idea
priority: 2
created: 2026-10-06T09:08:25Z
updated: 2026-10-06T10:44:19Z
depends: []
parent: tasks-984769
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

Task closed with done in the main checkout while its park was keyed to a since-removed worktree; prime shows it under parked, list --parked does not; no unpark command exists, so only hand-editing the state store would clear it.

## Notes

- 2026-10-06T09:45:48Z (main): scope: briefed; grouped record visibility and recovery evidence under tasks-984769; bounded reproductions precede a routing or recovery change; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-06T10:41:53Z (main): Human review probe: with isolated config and two independent state directories, a worktree park under state A survived closure under state B and record integration; prime under A showed the closed record as parked. Re-running tasks done <id> under A exited 0 and cleared its park. This uses host-local XDG_STATE_HOME, not a new cleanup command. Scratch probe was deleted; b9b10e must verify exact record changes and recurrence/foreign-claim safeguards.
- 2026-10-06T10:44:19Z (main): Additional isolated current-binary probe: one non-recurring record shared by two XDG_STATE_HOME stores; park under A, done under B leaves A park, then plain done under A exits 0 and clears it. Only task.updated changes; completed, model and notes remain unchanged. This probe used shared files without a Git worktree; removed-worktree, recurrence and foreign-live safeguards remain in b9b10e. Temporary config/state/project removed.
