---
id: tasks-2c7979
title: A park overlay that outlives its task's close stays in prime's parked section with no direct way to clear it; only a recompletion (done on a done task) removes it
status: dropped
priority: 2
created: 2026-10-06T18:13:29Z
updated: 2026-10-20T12:21:28Z
depends: []
parent: tasks-984769
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

prime lists a done task under parked (list --parked omits it). The only clearing path found is re-running tasks done, which adds a close note and restamps model unless TASKS_MODEL is unset. Expected: done/drop by another session clears it, or an explicit unpark.

## Notes

- 2026-10-08T10:12:57Z (main): scope: drop; duplicate of tasks-6b2867, covered by tasks-b9b10e; current code and its attached probe preserve completion/model on an already-done non-recurring retry, while an explicit message can append a note; original report retained; brief: docs/notes/2026-10-06-record-continuity-brief.md; proposal: drop as covered by tasks-6b2867 and tasks-b9b10e
- 2026-10-09T13:50:43Z (probe/park-recovery): tasks-b9b10e finding: a bare retry and a retry with a different TASKS_MODEL leave model, completed and notes unchanged; only an explicit message appends a note. Restamping not reproduced; drop proposal stands.
- 2026-10-09T15:27:39Z (main): dropped
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T15:27:39Z (main): Duplicate of tasks-6b2867 (user approved 2026-10-09); covered by tasks-b9b10e's probe, tasks-6a0050 and tasks-081099; original report retained
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-20T12:02:30Z (main): audit follow-up (tasks-fa3525): judge was wrong, tasks-b9b10e held both asks. Restamping was named in its body ("Compare a bare retry with an explicitly supplied close message and with TASKS_MODEL set to a different value; tasks-2c7979 reports note/model restamping") and disproved in its done note ("repeated done/drop safely clears a closed park, changing only updated; ... model is never restamped"); src/commands/mod.rs stamps model only when status changes to done. The explicit unpark was weighed and declined there ("No product code or new cleanup command"), and tasks-081099 (49bca87) made prime warn and name `tasks done <id>` / `tasks drop <id>` as the clearing command. Re-checked 2026-10-20 on tasks 0.4.0 in a two-store scratch probe: re-running done on the closed record with a different TASKS_MODEL cleared the park and changed only updated; with a message it appended one note and kept model. `tasks release` does not clear a park.
- 2026-10-20T12:21:28Z (main): addendum to the audit follow-up: an explicit unpark for open tasks is tasks-8e365c (todo, planned, filed 2026-10-17), which states that parks on closed records are covered by a repeated done.
