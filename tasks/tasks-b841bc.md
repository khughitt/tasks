---
id: tasks-b841bc
title: Cross-project task references for shared briefs and artifacts
status: idea
priority: 3
complexity: high
process: planned
created: 2026-10-19T15:11:58Z
updated: 2026-10-19T15:11:59Z
depends: []
tags: []
source: "tasks-3a445e:followup-cross-project-refs"
agent: codex
---

Why: task references currently reject a leading @, so a task cannot hold a structured reference to another registered project’s brief or artifact. Cross-project scope work already shares briefs through prose and audit notes. Capture the deferred extension from docs/specs/2026-10-19-task-references-design.md section 11.

Scope to settle: project-qualified syntax and canonical identity, registry aliases and prefix renames, project/document-root selection, linked checkouts and worktree/main precedence, unavailable-project diagnostics, and how writing, doc, inventory, check and list filtering share the same resolution contract. Coordinate with tasks-dc16eb (document roots), tasks-a73dce (branch-only document resolution), and tasks-91c622 (cross-project scoping); do not duplicate those tasks or assume their migrations must finish before design.

Done when scoped and, if pursued, a reviewed design defines cross-project reference behavior and compatibility/rollout. Do not copy shared briefs between projects or introduce an implementation before that design.

## Notes

- 2026-10-19T15:11:59Z (main): concerns: tasks-3a445e extension — support structured references to documents held by another project
