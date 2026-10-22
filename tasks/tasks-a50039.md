---
id: tasks-a50039
title: Resolve existing tilde-layout findings before checker rollout
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-22T19:07:48Z
updated: 2026-10-22T19:07:48Z
depends: []
tags: []
source: ops-295f8d
agent: codex
---

Why: the final candidate from ops-295f8d reports existing home-relative machine-layout paths that the current checker misses. Resolve these before the stricter vendored copy arrives, so unrelated commits are not blocked. These are scanner findings to classify, not an instruction to discard historical documents.

Done: inspect each listed file in context. Replace accidental checkout/storage paths in shared docs, examples or fixtures with repository-relative paths, project-role references or portable URLs as appropriate. For intentional host configuration, use only the narrow existing layout_allowed mechanism where this repository is host configuration by design; do not exempt ordinary docs wholesale. Preserve historical evidence, attachments and every spec/plan review ledger; follow the task attachment CLI and retention rules instead of editing attached artifacts by hand. If a runtime-path correction needs an unsettled public contract, change that piece to planned before implementation. Run the project-declared checks and the reviewed candidate checker by explicit path, then record every listed file disposition and the clean verdict on this task. Do not repoint shared launchers, vendor the checker, publish, push or activate global guidance in this task.

Where: the current project guide/profile, listed tracked files, existing layout_allowed, and ops-295f8d for candidate branch/path and canonical report. The full sweep reached 36 registered repositories with no unavailable roots; counts here are capped scanner records, not an estimate of all occurrences. No source snippets are copied.

Reported files:
- docs/notes/2026-10-08-claim-guard-cutover.md: first reported line 29; 2 reported finding(s)
- docs/plans/2026-08-29-tasks.md: first reported line 1126; 1 reported finding(s)
- docs/plans/2026-08-30-project-tasks-migration.md: first reported line 0; 16 reported finding(s)
- docs/plans/2026-08-31-material-prism-tasks-migration.md: first reported line 0; 12 reported finding(s)
- docs/plans/2026-09-13-creation-provenance.md: first reported line 631; 1 reported finding(s)
- docs/plans/2026-10-03-lanes-needs-groups.md: first reported line 11369; 1 reported finding(s)
- docs/plans/2026-10-06-project-resolution.md: first reported line 51; 4 reported finding(s)
- docs/plans/2026-10-07-linked-projects.md: first reported line 91; 1 reported finding(s)
- docs/plans/2026-10-08-claim-guard-policy.md: first reported line 345; 4 reported finding(s)
- docs/specs/2026-08-29-tasks-design.md: first reported line 783; 2 reported finding(s)
- docs/specs/2026-08-31-material-prism-tasks-migration-design.md: first reported line 43; 2 reported finding(s)
- docs/specs/2026-10-06-project-resolution-design.md: first reported line 268; 2 reported finding(s)
- src/locations.rs: first reported line 590; 1 reported finding(s)
- src/registry.rs: first reported line 671; 2 reported finding(s)
