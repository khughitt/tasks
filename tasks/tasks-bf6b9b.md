---
id: tasks-bf6b9b
title: Design individual task migration from the project-movement brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: task-migration-design
created: 2026-10-08T10:42:11Z
updated: 2026-10-09T01:44:53Z
started: 2026-10-09T01:02:06Z
depends: []
parent: tasks-a7460c
tags: []
source: docs/notes/2026-10-08-project-discovery-movement-brief.md
agent: codex
spec: docs/specs/2026-10-08-task-move-design.md
plan: docs/plans/2026-10-09-task-move.md
---

Why: tasks-7ba557 needs moving individual tasks between registered projects without losing history or breaking references. Project-wide prefix aliases cannot represent that operation.

Where to start: docs/notes/2026-10-08-project-discovery-movement-brief.md; src/registry.rs::canonical_id, src/resolve.rs::Resolver::resolve_task, src/commands/add.rs, src/hierarchy.rs::validate_parent, src/attachments.rs, src/claims.rs and src/rename/; the implemented project-resolution, prefix-rename and attachment specs.

Bound: produce a design spec and then an implementation plan, each reviewed by the user. Compare the existing source-and-title add/drop workflow with explicit migration to a new destination id and with per-task forwarding. Decide whether transparent old-id resolution is required before adding an identity mechanism. Settle destination collisions, inbound dependencies (including unavailable projects), parent/child boundaries, notes and provenance, attachment ledgers and bytes, spec/plan links, claim/park ownership, multi-host registry limits, partial failure and reruns. Reuse the shared registry/resolver boundaries; do not alias a still-live source prefix or silently route writes. No live migration or implementation in this task.

Done: reviewed design and plan state preserved versus intentionally changed identity/history, the smallest supported move, refusal/recovery behavior and acceptance cases. Include a single task with an inbound foreign dependency and attachment, a parented task, a live claim, a destination collision and interrupted retry. File implementation children only after plan review; retaining a documented add/drop workflow is a valid explicit disposition if it meets the agreed need.

Ideas it wakes: on completion, add the approved finding and artifact paths to tasks-7ba557 with tasks note in the same commit and update the brief so scope can reconsider it.

## Notes

- 2026-10-09T01:02:06Z (main): started
  provenance: {"harness_session":"claude-code:a4be4ef6-c289-4fd4-ba16-2df145dae2b5","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T01:02:16Z (task-migration-design): resumed
  provenance: {"harness_session":"claude-code:a4be4ef6-c289-4fd4-ba16-2df145dae2b5","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T01:15:53Z (task-migration-design): design: old id stays a read-only 'moved' tombstone with a pointer (user choice); inbound depends follow the pointer instead of being rewritten (deviation from the option text, argued in spec §2/§3.4); parent/children/claims refuse; journal+classify recovery modeled on rename; evidence: 9 past add/drop moves, none kept source notes
- 2026-10-09T01:16:16Z (task-migration-design): parked (waiting on user, review): user reviews docs/specs/2026-10-08-task-move-design.md (in .worktrees/task-migration-design); on approval, agent writes the implementation plan with writing-plans
  provenance: {"harness_session":"claude-code:a4be4ef6-c289-4fd4-ba16-2df145dae2b5","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T01:22:05Z (task-migration-design): review: spec round 1 — verdict: revise; findings: P1 1, P2 4; reviewer: codex
- 2026-10-09T01:25:28Z (task-migration-design): spec round 1 revised: undo restores from journal source_text and deletes last; cycle walks treat a tombstone's only edge as moved_to; carried park worktree = destination resume_root; check splits move_incomplete (reachable) from move_counterpart_unreachable (warning); journal freezes rename/retire/unretire/unregister/init --force, registry lock after sorted prefix locks; acceptance cases 3, 7-9 added
- 2026-10-09T01:29:50Z (task-migration-design): review: spec round 2 — verdict: revise; findings: P2 2; reviewer: codex
- 2026-10-09T01:30:56Z (task-migration-design): spec round 2 revised: journal keeps source_park/escalation verbatim plus rendered target entries; undo returns the originals; forward recovery requires a copy matching every attachment digest, else refuse and keep the journal until a verified backup is restored; acceptance cases 10-11 added
- 2026-10-09T01:32:27Z (task-migration-design): review: spec round 3 — verdict: accept; findings: none; reviewer: codex
- 2026-10-09T01:35:47Z (task-migration-design): resumed
  provenance: {"harness_session":"claude-code:a4be4ef6-c289-4fd4-ba16-2df145dae2b5","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T01:44:52Z (task-migration-design): plan drafted: docs/plans/2026-10-09-task-move.md, 8 tasks (schema; tombstone refusals; pointer-following deps/cycles; check findings; journal+classify; move fresh path; resume/explain/freeze; docs). Step children to be filed after plan review
