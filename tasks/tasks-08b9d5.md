---
id: tasks-08b9d5
title: "Park: record the next step and who it waits on when a task is set down"
status: idea
priority: 2
created: 2026-09-09T12:31:46Z
updated: 2026-09-09T12:39:44Z
depends: []
tags: [cli, observability, provenance]
---

Today 'doing' is the only active state, so a task set down overnight says nothing about whether it waits on the user or the agent, or what the next concrete action is. The current doing task shows the shape of the problem: its resumption state exists, but buried in the last of several long notes, invisible in every list view.

Proposal: 'tasks park <id> "<next step>" [--waiting-on user|agent]'. One command at the moment work already stops, no new Status variant, so the JSON contract, ready, closeout, and existing consumers stay untouched. 'blocked' is the wrong home: it hides a task from ready, while a parked task should sit at the top of the morning list. 'next' and 'prime' list parked tasks first with the next-step line.

Phase is derived, not stored. No spec means brainstorming; a spec without a plan means planning; a plan with open step children means implementing. The parked listing shows the derived phase beside the line, giving the observability without a field to keep honest.

Session capture rides on park. The claim store already resolves owner, session id, host, worktree, and started, but deliberately discards them when the claim clears. Park copies harness and session id into the park record, the one moment the session is worth remembering, so 'start' and 'done' write nothing new. Store the session as an opaque string with a harness tag, never interpreted, like --source: resume is harness-specific and perishable, so the launcher decides what to do with it. The human handle is the task title plus the next-step line; a session name is nice but unreliable.

Open questions: does park live in the record (a field, a structured note) or beside the claim; does re-parking replace or append; does start or done clear the park; what prime shows when a parked task is also live-claimed.

Consumers: the quick-launch picker (tasks-202e1f) and the familiar-side session-end hook that parks a still-claimed task automatically.

## Notes

- 2026-09-09T12:31:46Z (main): Follow-up quick-launch picker filed as tasks-202e1f; session capture as tasks-abfd3d.
- 2026-09-09T12:39:44Z (main): Reshaped from phase states to a park command; tasks-abfd3d (session capture) folded in. Hook piece filed in familiar.
- 2026-09-09T12:39:44Z (main): Familiar-side hook piece: fam-5b276b.
