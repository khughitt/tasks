---
id: tasks-964c95
title: "Centralize task filtering and add priority, size, complexity, and process filters"
status: doing
priority: 2
size: m
complexity: mid
process: planned
owner: feat/task-filters
created: 2026-09-30T16:40:28Z
updated: 2026-09-30T16:52:02Z
started: 2026-09-30T16:40:36Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-30-task-filters-design.md
---

list filters only on status, tag, owner, source, and parent; ready has --size (exact) and --parallel; ready/next have --max-complexity (a ceiling on effective complexity that hides unassessed). Nothing filters on priority or process.

Filter predicates are written inline where they are used: list's retain closure over Task (src/commands/list.rs, list) and list_parked's copy over ParkedRow; ready's size/parallel retains, each run twice (ready and deferred); the default open-not-shelved visibility repeated with small differences in list, hierarchy, graph, quiet, status, and tags. Only complexity has one home (src/complexity.rs: cutoff/effective/apply).

Goal: one TaskFilter in query.rs (or its own module) with matches(&Task, &ClaimSnapshot), a flattened FilterArgs shared by list and ready (and wherever else fits), list --parked filtering through the same predicate, and new filters for priority, size, complexity, and process. Complexity filtering must reuse complexity::effective. Open design points: exact vs range semantics for priority and complexity, how to select unassessed/unset values, whether the default visibility rule joins the filter, and JSON/CLI compatibility of the existing flags.

## Notes

- 2026-09-30T16:40:36Z (feat/task-filters): started
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T16:43:24Z (feat/task-filters): spec drafted: one TaskFilter (src/filter.rs) over a Fields view of Task and ParkedRow; list and ready share FilterArgs; none selects unset; complexity matches the effective rating; ready filters at candidacy; cli.toml rows change in ops first
- 2026-09-30T16:43:40Z (feat/task-filters): parked (waiting on user, review): user reviews docs/specs/2026-09-30-task-filters-design.md; on approval write the implementation plan in .worktrees/task-filters
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T16:48:10Z (feat/task-filters): review: spec round 1 — verdict: revise; findings: P2 3; reviewer: codex
- 2026-09-30T16:52:01Z (feat/task-filters): resumed
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T16:52:01Z (feat/task-filters): spec round 1 findings all verified and addressed: separate default pools for list and list --parked (shelved kept); TaskFilter::parse fallible for --parent invalid_id; verification lists the changed cutoff-composition and completion assertions, amends complexity §4.1, adds cutoff/selection intersection tests
