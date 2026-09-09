---
id: tasks-08b9d5
title: "Park: record the next step and who it waits on when a task is set down"
status: todo
priority: 2
size: l
created: 2026-09-09T12:31:46Z
updated: 2026-09-09T14:37:50Z
depends: []
tags: [cli, observability, provenance]
spec: docs/specs/2026-09-09-park-design.md
---

Today 'doing' is the only active state, so a task set down overnight says nothing about whether it waits on the user or the agent, or what the next concrete action is. The resumption state exists, but buried in the last of several long notes, invisible in every list view.

Decision (docs/specs/2026-09-09-park-design.md): 'tasks park <id> "<next step>" [--waiting-on user|agent]' converts the caller's hold into a park entry in the shared claim store: at, next step, waiting on, tagged session, owner, host, worktree, title snapshot. The record gains only a note. Status is untouched; any open task can be parked. start replaces the entry with a live claim (resume, including doing to doing); done and drop remove it; status-preserving saves leave the store alone but keep every guard. Phase is derived from spec and plan links, never stored. prime gains a parked section, ready omits user-parked work with a warning, next prefers agent-parked candidates (open, not blocked, dependencies closed, no children; an idea means resume scoping), list --parked feeds pickers. Store-only entries resolve through their recorded worktree and are display-only when unresolved.

Rejected: new Status variants (phase is derivable; contract churn) and frontmatter park fields (released claims lose cross-worktree visibility; a record field cannot represent cancellation and resurrects cleared parks).

Consumers: the quick-launch picker (tasks-202e1f) and the familiar session-end hook (fam-5b276b).

## Notes

- 2026-09-09T12:31:46Z (main): Follow-up quick-launch picker filed as tasks-202e1f; session capture as tasks-abfd3d.
- 2026-09-09T12:39:44Z (main): Reshaped from phase states to a park command; tasks-abfd3d (session capture) folded in. Hook piece filed in familiar.
- 2026-09-09T12:39:44Z (main): Familiar-side hook piece: fam-5b276b.
- 2026-09-09T13:20:02Z (park): Brainstormed 2026-09-09; design in docs/specs/2026-09-09-park-design.md. Next: writing-plans, then children per plan step.
- 2026-09-09T14:12:07Z (park): Spec revised after review 2026-09-09: store-authoritative parking, no frontmatter fields; start is resume; next eligibility and store-only rows defined.
- 2026-09-09T14:30:30Z (park): Second review 2026-09-09 folded in: store-only entries resolve by scanning the originating checkout and are never next candidates; rename migrates park entries; editor-save rule scoped to park preservation; unresolved row contract.
- 2026-09-09T14:37:50Z (park): Third review 2026-09-09: rename preflight rejects unrelated target parks; inventory records the expected target store (store_to) and recovery verifies it. Spec approved for planning.
