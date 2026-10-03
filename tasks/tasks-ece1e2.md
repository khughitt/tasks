---
id: tasks-ece1e2
title: Design project groups and goal focus from the work-selection brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: ece1e2-work-selection
created: 2026-09-30T10:04:57Z
updated: 2026-10-03T10:26:59Z
started: 2026-10-03T09:53:20Z
depends: []
parent: tasks-46d207
tags: []
agent: codex
spec: docs/specs/2026-10-03-lanes-needs-groups-design.md
---

Why: tasks-77dbc6 and tasks-9bdd68 need a shared contract for narrowing the project pool and preferring a goal without overriding eligibility.
Where to start: docs/notes/2026-09-30-work-selection-brief.md; src/scope.rs, src/registry.rs, src/query.rs, src/commands/list.rs::next and ready_tasks, src/commands/parked.rs::candidates; multi-project and park design specs.
Bound: compare explicit registry groups with repeated project selection; compare opt-in goal preference with current priority ordering. Set membership, alias/rename/unregister behavior, state location, precedence against parked-agent resumes, and closed/missing focus behavior. The original mindful reference is unavailable; its captured task body is the available source. No implementation during design.
Done: one reviewed design with concrete picker examples, preserved claim/dependency/complexity/deferral gates, command and JSON changes named, and independent implementation slices; obtain the implementation-plan review before code.
Ideas it wakes: record the resulting decisions on tasks-77dbc6 and tasks-9bdd68 and update the brief in the same commit as the result.

## Notes

- 2026-10-03T09:53:20Z (main): scope: widened at user request (2026-10-03) to cover tasks-e02860: an ordered set of active goals (lanes) rather than one focus, lane guidance and next step surfaced in prime, and per-task resource needs (generalising park's needs) from which the picker derives which lanes can run now. The design must still settle project groups (tasks-77dbc6).
- 2026-10-03T09:53:20Z (main): started
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-03T09:53:25Z (ece1e2-work-selection): resumed
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-03T10:07:48Z (ece1e2-work-selection): review: spec round 1 — verdict: revise; findings: P1 3, P2 7, P3 10; reviewer: claude-code/claude-opus-5-5
- 2026-10-03T10:15:37Z (ece1e2-work-selection): review: spec round 2 — verdict: revise; findings: P2 4, P3 7; reviewer: claude-code/claude-opus-5-5
- 2026-10-03T10:15:46Z (ece1e2-work-selection): parked (waiting on user, review): User reviews the spec docs/specs/2026-10-03-lanes-needs-groups-design.md (rounds 1-2 applied); on approval, write the implementation plan in this worktree with writing-plans, four slices per spec §13
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-03T10:26:35Z (ece1e2-work-selection): resumed
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-03T10:26:59Z (ece1e2-work-selection): review: spec round 3 — verdict: revise; findings: P2 3; reviewer: external (pasted by user)
