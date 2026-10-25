---
id: tasks-92cf3a
title: Align shared filter and tree completion
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: design/tasks-284d9b
created: 2026-10-25T11:14:20Z
updated: 2026-10-25T11:56:09Z
started: 2026-10-25T11:56:09Z
depends: [tasks-3b3f0b]
parent: tasks-284d9b
tags: []
agent: codex
spec: docs/specs/2026-10-25-filtered-read-routing-design.md
plan: docs/plans/2026-10-25-filtered-read-routing-plan.md
step: "Task 2: Align shared filter and tree completion"
---

Keep complete::scoped shared by ID filters and tree, using local_or_foreign only without explicit scope. Follow plan Task 2's foreign-prefix, local/worktree, suffix, empty-fragment, explicit-scope and silent-error tests; update the old tree completion assertion and current docs. Execute after Task 1 and parent plan approval, in the parent's existing worktree. Parent owns retained resources.

## Notes

- 2026-10-25T11:56:09Z (design/tasks-284d9b): started
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
