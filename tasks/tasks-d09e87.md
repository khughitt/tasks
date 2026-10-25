---
id: tasks-d09e87
title: Design cross-project reference resolution from the extensions brief
status: todo
priority: 3
size: m
complexity: high
process: planned
created: 2026-10-20T01:40:45Z
updated: 2026-10-25T15:49:35Z
depends: []
parent: tasks-7102c1
tags: []
source: tasks-b841bc
agent: codex
---

Why: structured references reject @ while shared briefs already cross project boundaries. [ref:brief] establishes the implemented contract and related work.

Bound: propose one project-qualified reference contract covering canonical project identity and aliases, prefix rename and task move, document-root selection, linked checkouts and worktree/main precedence, unavailable-project diagnostics, and consistent add/edit, inventory, doc, check, and exact list filtering. Coordinate tasks-dc16eb, tasks-a73dce/tasks-6360ac, and tasks-91c622; consume existing investigations rather than repeat them. No copied briefs, new artifact store, or document-root migration in this task.

Done: a written design spec and implementation plan reviewed by the user, with acceptance cases for ordinary and linked projects, aliases/rename, absent projects/documents, and worktree/main differences. Explicitly resolve any prerequisite decision still open in related work before implementation. New CLI options, if any, begin with ops vocabulary rows.

Ideas it wakes: On completion, run `tasks note tasks-b841bc --no-reviewed "<finding>"` and `tasks note tasks-041759 --no-reviewed "<finding>"`; update both input briefs before this result's branch merges.

Additional acceptance case: [ref:evidence-brief] records a scope handoff that reuses a follow-up in another project. The follow-up must declare its authoritative brief as required input and retrieve it without an absolute host path or an attached duplicate. Cover add/edit, inventory, doc and check when that project or document is unavailable; distinguish a live authoritative reference from an intentional dated snapshot. This is the existing cross-project reference design, not a second design effort.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — question, evidence, and scope boundaries
- evidence-brief (input, required, kind=brief): docs/notes/2026-10-25-evidence-handoffs-brief.md — cross-project required-input acceptance case
