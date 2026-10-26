---
id: tasks-ba4528
title: Design first-occurrence scheduling from the brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-26T15:35:48Z
updated: 2026-10-26T15:35:48Z
depends: []
parent: tasks-46d207
tags: []
source: "tasks-adf58d:first-occurrence-design"
agent: codex
origin: agent
---

Why: tasks-adf58d requests a chosen first occurrence for a recurring task; current code deliberately rejects every with defer. Read [ref:brief] first for the observed refusal, originating context, alternatives and constraints.

Where to start: docs/specs/2026-09-15-defer-design.md section 2.4; docs/specs/2026-09-09-periodic-design.md; src/format.rs::validate_task, src/periodic.rs::due/is_due, src/defer.rs, src/query.rs, src/commands/edit.rs and status-transition code; tests/cli.rs::defer_refuses_bad_values_and_incompatible_records. Coordinate with tasks-157d05's parked-work date design without extending this task to parked-doing resumption.

Bound: compare reusing defer for an initial occurrence, a separate first-occurrence field/option, and the current manual workflow. Specify before/at/after-date examples, first/early start and completion, reopen and cadence removal/re-addition, truthful completion stamps, editor parity, picker/reporting behavior, UTC boundary and binary rollout compatibility. Preserve existing recurrence defaults and completion-based later cycles. Do not implement runtime code or add a scheduler. Any selected new CLI surface must follow the ops-first adoption rule.

Done: obtain user review of a written design spec, then user review of its implementation plan before implementation. State the chosen representation, lifecycle/validation rules, affected views, compatibility/rollout requirements and focused regression cases. Update this task and the brief with the approved result.

Ideas it wakes: on completion run tasks note tasks-adf58d --no-reviewed "<approved finding>" and update [ref:brief] before this result's branch merges. Keep the dependency from tasks-adf58d to this task.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-26-first-recurrence-date-brief.md — observed refusal, alternatives and lifecycle questions

## Notes

- 2026-10-26T15:35:48Z (add): created
  provenance: {"harness_session":"codex:01a178d6-0f5f-7d01-8ac1-a01cfe262c14","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
