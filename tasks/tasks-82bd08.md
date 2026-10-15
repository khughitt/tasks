---
id: tasks-82bd08
title: Establish where a branch-only spec or plan can be linked and checked
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-15T16:55:24Z
updated: 2026-10-15T17:32:40Z
depends: []
parent: tasks-a73dce
tags: []
agent: claude-code
---

Question: Can a planned task in an in-tree project link its spec, plan and step children while those documents exist only on its branch, by writing the records in the task's worktree, and what forced mind6's plan execution to write step records in the main checkout instead?
Where to start: docs/notes/2026-10-15-branch-only-document-links-brief.md; Resolver::locate/resolve_doc in src/resolve.rs; doc_missing in src/commands/check.rs; mind6-3a9a27's step children and mind6's AGENTS.md and flow guidance; tasks-325e79.
Bound: one disposable in-tree project with a worktree, isolated XDG_CONFIG_HOME and XDG_STATE_HOME. Write a spec and plan on the branch only; try edit --spec/--plan and add --plan --step from the worktree and from main; run check in both checkouts before and after the merge. Read mind6's guidance for why records were written in main. Linked projects are not exercised here: their code-checkout documents are designed in tasks-6360ac, which takes this finding for a document that exists only on an unmerged branch. No product-code change.
Expected result: record on this task and in the brief whether writing records in the worktree is a supported workflow, the exact sequence, or the precise gap with acceptance cases for a planned design task under tasks-a73dce.
Ideas it wakes: On completion, run `tasks note <id> --no-reviewed "<finding>"` on tasks-cefa7d before this result's branch merges.
