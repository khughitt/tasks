---
id: tasks-a9b8d3
title: Design explicit idea review state from the work-selection brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-08T10:53:28Z
updated: 2026-10-08T10:53:28Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-e34b99
agent: codex
---

Why: tasks-e34b99 requests visible idea-review state, and its 2026-10-08 user input chooses a top-level boolean and a left-most * marker for unread ideas. Scope currently rereads the latest note to determine eligibility.
Where to start: docs/notes/2026-09-30-work-selection-brief.md; tasks-e34b99 original report and user-input note; src/model.rs, src/frontmatter.rs, src/cli.rs, src/filter.rs, src/output.rs, src/commands/edit.rs, src/commands/note.rs, src/commands/sample.rs, skills/scope/SKILL.md and skills/curate/SKILL.md.
Bound: Design the smallest explicit reviewed flag, list filtering and pretty marker contract. Preserve the requested boolean direction; distinguish the user's choice from the agent's proposed reviewed name, explicit wake reset, and one-time latest-note backfill. Settle unset/false/true serialization, non-idea transitions and return to idea, atomic note-plus-reset behavior, safe rerunnable migration, and pending drop proposals versus parks. Do not equate idea review with design/plan approval or executable status. Identify affected JSON consumers, fixtures, CLI completion, skills and docs; coordinate eligibility semantics with tasks-91c622 without expanding into cross-project discovery.
Done: A written spec reviewed by the user, followed by a reviewed implementation plan. Acceptance examples cover new, briefed, explicitly readmitted and repeatedly reviewed ideas; incidental later notes; pending proposals; non-idea transitions; migration reruns; JSON/filter/pretty agreement; and stale-copy/live-claim refusal. No implementation or live backfill during this design task.
Ideas it wakes: On completion, record the approved finding with tasks note on tasks-e34b99 and update the brief in the same commit. Use the current note-based wake rule until an explicit review-state contract has actually shipped.
