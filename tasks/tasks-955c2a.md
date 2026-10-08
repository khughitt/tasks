---
id: tasks-955c2a
title: "tasks note refuses with stale_copy when another live session holds a newer copy of the record, though notes are append-only and cannot conflict; the only offered paths are waiting for that branch to merge or merging its copy"
status: idea
priority: 2
created: 2026-10-07T17:45:57Z
updated: 2026-10-08T09:44:40Z
depends: []
parent: tasks-984769
tags: [feedback, friction, "from:flows"]
agent: claude-code/claude-opus-5-5
---

Command: tasks note <id> "<text>" from one checkout while another live session holds <id> in its own worktree. Error kind: stale_copy ('nothing was written … wait for that branch to merge, or merge its copy into this checkout'). Expected: an append-only note does not need the holder's copy to be current, so it should not be refused like a field edit. Suggestion: queue the note in the shared store beside claims, addressed to the task; the holder's next tasks command on that task folds it into its copy (and show/prime surface it meanwhile), so the holder's branch carries it into git. Writing directly into the holder's checkout instead would race its own read-modify-write. Workaround used: dropped the note and relayed its content by hand.

## Notes

- 2026-10-08T09:44:40Z (main): scope: briefed; preserved the full queued-note proposal and grouped under tasks-984769; tasks-a984f1 will probe explicit current-checkout delivery, whole-record locking, claim preservation and retries before any queue design; brief: docs/notes/2026-10-06-record-continuity-brief.md
