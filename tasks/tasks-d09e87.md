---
id: tasks-d09e87
title: Design cross-project reference resolution from the extensions brief
status: todo
priority: 3
size: m
complexity: high
process: planned
created: 2026-10-20T01:40:45Z
updated: 2026-10-20T01:40:45Z
depends: []
parent: tasks-7102c1
tags: []
source: tasks-b841bc
agent: codex
---

Why: structured references reject @ while shared briefs already cross project boundaries. [ref:brief] establishes the implemented contract and related work.

Bound: propose one project-qualified reference contract covering canonical project identity and aliases, prefix rename and task move, document-root selection, linked checkouts and worktree/main precedence, unavailable-project diagnostics, and consistent add/edit, inventory, doc, check, and exact list filtering. Coordinate tasks-dc16eb, tasks-a73dce/tasks-6360ac, and tasks-91c622; consume existing investigations rather than repeat them. No copied briefs, new artifact store, or document-root migration in this task.

Done: a written design spec and implementation plan reviewed by the user, with acceptance cases for ordinary and linked projects, aliases/rename, absent projects/documents, and worktree/main differences. Explicitly resolve any prerequisite decision still open in related work before implementation. New CLI options, if any, begin with ops vocabulary rows.

Ideas it wakes: On completion, run `tasks note tasks-b841bc --no-reviewed "<finding>"` and update the brief before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — question, evidence, and scope boundaries
