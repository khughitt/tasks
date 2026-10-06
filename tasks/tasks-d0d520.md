---
id: tasks-d0d520
title: Determine which checkout cross-project dependencies should read
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T09:45:48Z
updated: 2026-10-06T09:45:49Z
depends: []
parent: tasks-984769
tags: []
source: tasks-70f0bc
agent: codex
---

Question: How do cross-project dependency creation and readiness read a worktree-only or newer completed task, and is an explicit publication step sufficient?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/resolve.rs::resolve_task/resolve_registered, src/commands/dep.rs::add_dependencies/ensure_acyclic, src/commands/list.rs::resolve_dependency, and the record-home and multi-project specs.

Bound: one disposable two-project git fixture with isolated XDG_CONFIG_HOME, XDG_STATE_HOME and RELAY_STATE_DIR; create an upstream worktree-only task, attempt a downstream dependency, integrate just its record, then close upstream in its worktree and compare downstream readiness before and after record integration. Compare show with dependency resolution. No routing, JSON, host configuration or product-code change.

Expected result: an observed behavior table and recommendation choosing explicit publication or a reviewed shared resolver change, including failure/cycle/foreign-claim checks and the owner of any workflow change; update the brief.

Ideas it wakes: on completion add the finding to tasks-70f0bc, tasks-562c53 with tasks note in the same commit, update the brief, then rerun scope on them.

## Notes

- 2026-10-06T09:45:48Z (main): concerns: tasks-9949f3 extension — Establish cross-project dependency visibility beyond the delivered local-show and stale-write contract.
