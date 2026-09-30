---
id: tasks-964c95
title: "Centralize task filtering and add priority, size, complexity, and process filters"
status: done
priority: 2
size: m
complexity: mid
process: planned
owner: feat/task-filters
created: 2026-09-30T16:40:28Z
updated: 2026-09-30T18:37:56Z
started: 2026-09-30T16:40:36Z
completed: 2026-09-30T18:37:56Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-30-task-filters-design.md
plan: docs/plans/2026-09-30-task-filters.md
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
- 2026-09-30T16:52:03Z (feat/task-filters): parked (waiting on user, review): user or reviewer re-reviews docs/specs/2026-09-30-task-filters-design.md (round 2); on approval write the implementation plan in .worktrees/task-filters
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T16:55:17Z (feat/task-filters): review: spec round 2 — verdict: accept; findings: P3 1; reviewer: codex
- 2026-09-30T16:55:17Z (feat/task-filters): Round 2: all three P2 findings resolved; accepted for implementation planning. Nonblocking P3: spec line 79 says invalid parent parsing occurs before scanning today, but list.rs scans at line 48 before parse_id at line 53 (list_parked does likewise). Keep pre-scan validation as the intended new behavior and remove the historical claim.
- 2026-09-30T16:56:53Z (feat/task-filters): resumed
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T17:02:28Z (feat/task-filters): plan drafted: docs/plans/2026-09-30-task-filters.md, 3 tasks (filter module + list; ready at candidacy + complexity §4.1 amendment; docs, review, ops rollout). Plan link and step children wait for plan approval so pickers do not offer unapproved steps
- 2026-09-30T17:02:30Z (feat/task-filters): parked (waiting on user, review): user reviews docs/plans/2026-09-30-task-filters.md and picks subagent-driven or native execution; then link the plan, file step children, and execute in .worktrees/task-filters
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T17:04:37Z (feat/task-filters): review: plan round 1 — verdict: revise; findings: P1 1, P2 2; reviewer: codex
- 2026-09-30T17:04:38Z (feat/task-filters): Plan review findings: P1 lines 1191-1197 use vendored check as a dirty-file check, then force-publish both cli.toml and cli_surface.py across registered checkouts; inspect destination git status/diffs for both files, stop on unrelated changes, and reconcile the ops branch before publishing. P2 lines 165-167 assume insertion order for equal-priority tasks, but list sorts by updated descending then random id; compare sorted ids. P2 lines 1160-1162 document none for priority and OR for repeated tag; state the priority and all-of tag exceptions.
- 2026-09-30T17:40:23Z (feat/task-filters): resumed
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T17:40:23Z (feat/task-filters): plan round 1 findings verified and addressed: publish now reconciles the ops worktree with main under its uncommitted edit, checks git status and diff of tools/cli.toml and tools/cli_surface.py in every destination and stops on any change, and verifies only cli.toml rows changed after publishing; escalation test compares sorted ids; skill text excludes none from --priority and keeps --tag all-of
- 2026-09-30T17:40:24Z (feat/task-filters): parked (waiting on user, review): plan round 2 review of docs/plans/2026-09-30-task-filters.md and an execution choice (native recommended); then link the plan, file step children, and execute in .worktrees/task-filters
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T17:55:06Z (feat/task-filters): review: plan round 2 — verdict: revise; findings: P2 2; reviewer: codex
- 2026-09-30T17:55:06Z (feat/task-filters): Plan round 2: previous test-order and skill-text findings resolved; dirty-destination checks now cover both published files. Remaining P2: lines 1236-1239 verify allowed content deltas only after force-publishing; clean git state does not prove destination helper equals the source or inventory differs only in the two intended rows. Check those byte/content comparisons before publishing and scope post-publish diffs to the two files. P2 lines 1214-1216 conflate the task worktree with main after integration; fast-forwarding main does not change the worktree branch. Reconcile ops and refresh/test the inventory in the task worktree before integrating tasks, then compare the registered main copy.
- 2026-09-30T18:02:05Z (feat/task-filters): resumed
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T18:02:05Z (feat/task-filters): plan round 2 findings verified and addressed: ops reconcile and inventory refresh happen in the task worktree before integration; the registered tasks copy is compared after the fast-forward; every destination is preflighted by content (helper equals the source, inventory equals ops main) before publishing; later diffs are scoped to the two vendor files
- 2026-09-30T18:02:07Z (feat/task-filters): parked (waiting on user, review): plan round 3 review of docs/plans/2026-09-30-task-filters.md and an execution choice (native recommended); then link the plan, file step children, and execute in .worktrees/task-filters
  provenance: {"harness_session":"claude-code:b8006f0d-3c43-4298-a25a-79f366ea400c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T18:10:34Z (feat/task-filters): resumed
  provenance: {"harness_session":"codex:01a0f380-ddb1-71c3-a720-bc7a2dc7b70e","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T18:10:52Z (feat/task-filters): review: plan round 3 — verdict: accept; findings: none; reviewer: human
- 2026-09-30T18:32:35Z (feat/task-filters): Filed tasks-ead38f to decide whether repeated --tag filters widen or the CLI inventory records an all-of exception.
- 2026-09-30T18:36:01Z (feat/task-filters): review: impl round 1 — verdict: accept; findings: minor 1; reviewer: codex/gpt-6-astra
- 2026-09-30T18:37:56Z (feat/task-filters): done
  provenance: {"harness_session":"codex:01a0f380-ddb1-71c3-a720-bc7a2dc7b70e","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T18:37:56Z (feat/task-filters): Shared filters for list and ready, documented and reviewed; task branch ready for integration and CLI inventory rollout
  provenance: {"harness_session":"codex:01a0f380-ddb1-71c3-a720-bc7a2dc7b70e","harness_session_source":"CODEX_SESSION_ID"}
