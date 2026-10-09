---
id: tasks-6b2867
title: "A park recorded from a worktree survives the task's close from another checkout, and no command clears it: prime keeps listing the closed task as parked"
status: idea
priority: 2
created: 2026-10-06T09:08:25Z
updated: 2026-10-09T13:50:44Z
depends: [tasks-6a0050, tasks-081099]
parent: tasks-984769
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

Why: A host-local park can survive task closure recorded through another host state store. Current prime and default list --parked disagree on displaying the closed record.

Established evidence: The attached isolated-store probe on tasks-b9b10e found that plain done on an already-done non-recurring task clears the local park and changes only updated, preserving completed, model and notes. Current transition code stamps completion metadata only on a fresh completion; close can append an explicitly supplied message. This corrects the original no-command-clears-it claim without establishing a universal recovery recipe.

Remaining decision: tasks-b9b10e must verify removed-worktree, recurrence and foreign-live-claim safety, distinguish a bare retry from one with a message or TASKS_MODEL set, and recommend consistent parked-view behavior. Reuse that P2/s/mid/direct investigation before choosing an implementation.

Where to look: docs/notes/2026-10-06-record-continuity-brief.md; src/commands/mod.rs::transition/save, src/commands/status.rs::close, src/commands/list.rs::list_parked, src/commands/parked.rs, src/claims.rs.

Original report (preserved): Task closed with done in the main checkout while its park was keyed to a since-removed worktree; prime shows it under parked, list --parked does not; no unpark command exists, so only hand-editing the state store would clear it.

## Notes

- 2026-10-06T09:45:48Z (main): scope: briefed; grouped record visibility and recovery evidence under tasks-984769; bounded reproductions precede a routing or recovery change; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-06T10:41:53Z (main): Human review probe: with isolated config and two independent state directories, a worktree park under state A survived closure under state B and record integration; prime under A showed the closed record as parked. Re-running tasks done <id> under A exited 0 and cleared its park. This uses host-local XDG_STATE_HOME, not a new cleanup command. Scratch probe was deleted; b9b10e must verify exact record changes and recurrence/foreign-claim safeguards.
- 2026-10-06T10:44:19Z (main): Additional isolated current-binary probe: one non-recurring record shared by two XDG_STATE_HOME stores; park under A, done under B leaves A park, then plain done under A exits 0 and clears it. Only task.updated changes; completed, model and notes remain unchanged. This probe used shared files without a Git worktree; removed-worktree, recurrence and foreign-live safeguards remain in b9b10e. Temporary config/state/project removed.
- 2026-10-06T10:59:44Z (main): Attribution correction for the 2026-10-06T10:41:53Z note beginning Human review probe: this was the reviewer's isolated probe performed at the controller's request, not an established human-run probe. Its technical result and cleanup report are unchanged; reviewer harness/model identity was not supplied.
- 2026-10-06T11:39:23Z (main): Review finding: prime lists a park on a closed record while list --parked hides it (list.rs::list_parked filters is_open; prime's parked::rows does not), matching this report's second symptom. Added to tasks-b9b10e's question; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-08T10:12:57Z (main): scope: briefed; corrected the unsupported no-recovery premise while preserving the original report; tasks-b9b10e retains removed-worktree, recurrence, ownership and view-consistency checks and will distinguish bare retries from explicit messages; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-09T13:50:43Z (probe/park-recovery): tasks-b9b10e finding: repeated done clears a closed non-recurring park after worktree removal, changing only updated, for both sync and git arrival; a live foreign claim refuses. Gaps: a recurring record's park cannot be cleared once its worktree is gone (tasks-6a0050), and prime lists closed parks that list --parked hides (tasks-081099). Brief: docs/notes/2026-10-06-record-continuity-brief.md
