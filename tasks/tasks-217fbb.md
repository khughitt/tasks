---
id: tasks-217fbb
title: Design read-only recovery hints for missing worktree tasks
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-25T16:42:50Z
updated: 2026-10-25T16:42:50Z
depends: []
parent: tasks-984769
tags: []
source: "tasks-399eef:recovery-design"
agent: codex
origin: agent
---

Why: tasks-399eef reports an apparently missing task whose only record is in an unclaimed, unparked Git worktree. [ref:brief] establishes the current claim/park-only fallback, accepted record-home visibility limit, alternatives and constraints at 220bb14. The diagnostic contract needs review before changing that limit.

Done: produce a written design for user review, then an implementation plan for user review. Prefer a read-only missing-record diagnostic with an explicit checkout retry over automatically returning a sibling record. Settle eligible project layouts, zero/unique/multiple candidates, missing and unreadable locations, malformed records, bounded scans and races. Preserve local and claim/park precedence, error kind and JSON shape unless the reviewed design explicitly changes them, write routing, ownership and registered dependency authority. Exclude historical records in detached code worktrees, other hosts, arbitrary clones, record copying/merging and runtime activation. Do not claim the original incident was reproduced by this scope pass.

Where to look: src/commands/show.rs::read_record/recorded_elsewhere; src/repo.rs::Project::sibling_task_copies/worktree_roots; src/stale.rs; src/link.rs and detached pairing; docs/specs/2026-09-30-record-home-design.md sections 6.1 and 8; existing show worktree tests in tests/cli.rs. Coordinate with tasks-f3910b (newer local-copy warnings), tasks-70f0bc (publication guidance) and tasks-7b1b22 (missing-checkout classification), without duplicating or depending on unrelated remedies.

Verification in the plan: isolated fixtures for no match, unique and multiple matches, incomplete scans, local/claim/park precedence, nested in-tree and detached/linked layouts, and unchanged write behavior. Check every caller before reusing the sibling helper. Keep Git config, registry and fixture environments isolated.

Ideas it wakes: On completion, run `tasks note tasks-399eef --no-reviewed "<reviewed design finding and implementation handoff>"` before this result's branch merges. This task supplies reviewed design and plan, not production implementation.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-missing-task-recovery-brief.md — current visibility limit and diagnostic decisions

## Notes

- 2026-10-25T16:42:50Z (add): created
  provenance: {"harness_session":"codex:01a173ed-fd7b-7fc3-8372-3ad7eca32e70","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
