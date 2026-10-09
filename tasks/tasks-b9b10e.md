---
id: tasks-b9b10e
title: Establish safe recovery for a park left on a closed task
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T09:45:48Z
updated: 2026-10-08T10:12:58Z
depends: []
parent: tasks-984769
tags: []
source: tasks-6b2867
agent: codex
---

Question: How can a park left on a closed task by another host's state store be recovered safely, and should prime and list --parked agree on showing it?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/commands/mod.rs::transition/claim_guard/save, ClaimIntent::Release clear_park, src/commands/parked.rs::rows, src/commands/list.rs::prime/list_parked and src/claims.rs::remove_park.

Bound: one disposable git main/worktree pair and isolated config plus two independent XDG_STATE_HOME directories representing hosts A and B; park in the worktree, integrate the record, close in main with the same and a distinct explicit session, inspect show/prime/list --parked (with and without --status done) and the scratch store, then remove the scratch worktree through git and retry existing recovery commands. Use CLI/git operations, never edit a live store. Record binary versions, cleanup warnings and whether the closed record arrived by done or git integration. Also park using state A, close the integrated record using state B, then inspect state A after Git integration (and after removing the recorded scratch worktree). Retry plain tasks done <id> on the closed non-recurring record under state A. Compare a bare retry with an explicitly supplied close message and with TASKS_MODEL set to a different value; tasks-2c7979 reports note/model restamping, whereas the existing probe and current transition code preserve completion metadata. Keep these variants isolated so their exact effects are attributable. Compare exact record bytes, updated/completed/model fields, lifecycle notes, claim/park state and warnings before and after the retry; distinguish recurrence and foreign live ownership safeguards. No product code or new cleanup command.

Expected result: verify the reviewer's finding of host-local stale park plus successful repeated done, state exactly what that retry writes, and distinguish successful cleanup, failed cleanup and a park paired with an already-closed merged record; record prime versus list --parked output for that park and recommend which should change (list_parked hides closed records via is_open, prime lists them), leaving any output change to a reviewed decision; recommend the smallest safe recovery and regression check, preserving valid parks and concurrent work; update the brief.

Ideas it wakes: on completion add the finding to tasks-6b2867 with tasks note in the same commit, update the brief, then rerun scope on them.

## Notes

- 2026-10-06T10:41:53Z (main): Human review probe: with isolated config and two independent state directories, a worktree park under state A survived closure under state B and record integration; prime under A showed the closed record as parked. Re-running tasks done <id> under A exited 0 and cleared its park. This uses host-local XDG_STATE_HOME, not a new cleanup command. Scratch probe was deleted; b9b10e must verify exact record changes and recurrence/foreign-claim safeguards.
- 2026-10-06T10:44:19Z (main): Additional isolated current-binary probe: one non-recurring record shared by two XDG_STATE_HOME stores; park under A, done under B leaves A park, then plain done under A exits 0 and clears it. Only task.updated changes; completed, model and notes remain unchanged. This probe used shared files without a Git worktree; removed-worktree, recurrence and foreign-live safeguards remain in b9b10e. Temporary config/state/project removed.
- 2026-10-06T10:44:19Z (main): attached: tasks-review-park-result.json (610 bytes): Two isolated host stores: repeated done clears local park and changes only updated on a non-recurring record; limits recorded.
- 2026-10-06T10:59:44Z (main): Attribution correction for the 2026-10-06T10:41:53Z note beginning Human review probe: this was the reviewer's isolated probe performed at the controller's request, not an established human-run probe. Its technical result and cleanup report are unchanged; reviewer harness/model identity was not supplied.
- 2026-10-08T10:12:57Z (main): Scope refinement: tasks-2c7979 duplicates tasks-6b2867; retain the existing bounded investigation and compare bare done with an explicit message and changed TASKS_MODEL to reconcile the new report with the attached probe. No new recovery command or research task.
