---
id: tasks-abfd3d
title: "Record the coding-agent session behind a task's work, with a resumable name"
status: dropped
priority: 2
created: 2026-09-09T12:31:46Z
updated: 2026-09-09T12:39:44Z
depends: []
tags: [cli, provenance]
---

The claim store already resolves a session identity (TASKS_SESSION, CLAUDE_CODE_SESSION_ID, or the Unix session id) but only to arbitrate ownership, and it is discarded when the claim clears. Capture that session information durably on the task: which harness, the session id, and if available a human name for the session, so a task can point back at the conversation that worked on it. Decide what is worth keeping in the record versus beside the claim, and whether 'start' captures it automatically or a 'session' subcommand attaches it. Primary consumer is quick launch: resume the session that left a task mid-phase instead of starting cold. Related: the provenance idea about recording the model and key parameters.

## Notes

- 2026-09-09T12:31:46Z (main): Consumer: quick-launch picker tasks-202e1f. Related provenance idea: tasks-222dab.
- 2026-09-09T12:39:44Z (main): Folded into tasks-08b9d5: session capture rides on the park record instead of a field of its own.
