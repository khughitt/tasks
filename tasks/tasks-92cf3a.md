---
id: tasks-92cf3a
title: Align shared filter and tree completion
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-25T11:14:20Z
updated: 2026-10-25T11:14:20Z
depends: [tasks-3b3f0b]
parent: tasks-284d9b
tags: []
agent: codex
spec: docs/specs/2026-10-25-filtered-read-routing-design.md
plan: docs/plans/2026-10-25-filtered-read-routing-plan.md
step: "Task 2: Align shared filter and tree completion"
---

Keep complete::scoped shared by ID filters and tree, using local_or_foreign only without explicit scope. Follow plan Task 2's foreign-prefix, local/worktree, suffix, empty-fragment, explicit-scope and silent-error tests; update the old tree completion assertion and current docs. Execute after Task 1 and parent plan approval, in the parent's existing worktree. Parent owns retained resources.
