---
id: tasks-2c7979
title: A park overlay that outlives its task's close stays in prime's parked section with no direct way to clear it; only a recompletion (done on a done task) removes it
status: idea
priority: 2
created: 2026-10-06T18:13:29Z
updated: 2026-10-09T13:50:43Z
depends: []
parent: tasks-984769
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

prime lists a done task under parked (list --parked omits it). The only clearing path found is re-running tasks done, which adds a close note and restamps model unless TASKS_MODEL is unset. Expected: done/drop by another session clears it, or an explicit unpark.

## Notes

- 2026-10-08T10:12:57Z (main): scope: drop; duplicate of tasks-6b2867, covered by tasks-b9b10e; current code and its attached probe preserve completion/model on an already-done non-recurring retry, while an explicit message can append a note; original report retained; brief: docs/notes/2026-10-06-record-continuity-brief.md; proposal: drop as covered by tasks-6b2867 and tasks-b9b10e
- 2026-10-09T13:50:43Z (probe/park-recovery): tasks-b9b10e finding: a bare retry and a retry with a different TASKS_MODEL leave model, completed and notes unchanged; only an explicit message appends a note. Restamping not reproduced; drop proposal stands.
