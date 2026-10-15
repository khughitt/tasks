---
id: tasks-82bd08
title: Establish where a branch-only spec or plan can be linked and checked
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-15T16:55:24Z
updated: 2026-10-15T16:55:24Z
depends: []
parent: tasks-a73dce
tags: []
agent: claude-code
---

Question: Can a planned task link its spec, plan and step children while those documents exist only on its branch, by writing the records in the task's worktree (in-tree and linked projects), and what forced mind6's plan execution to write step records in the main checkout instead?
Where to start: docs/notes/2026-10-15-branch-only-document-links-brief.md; Resolver::locate/resolve_doc in src/resolve.rs; doc_missing in src/commands/check.rs; parked::open_recorded for linked projects; mind6-3a9a27's step children and mind6's AGENTS.md and flow guidance; ops docs/specs/2026-10-01-linked-checkouts-design.md section 4; tasks-325e79.
Bound: one disposable in-tree project and one linked project (overlay plus code repo), each with a worktree, isolated XDG_CONFIG_HOME and XDG_STATE_HOME. In each, write a spec and plan on the branch only; try edit --spec/--plan and add --plan --step from the worktree and from main; run check in both checkouts before and after the merge. Read mind6's guidance for why records were written in main. No product-code change.
Expected result: record on this task and in the brief which alternative serves each case, the exact supported workflow, or the precise gap with acceptance cases for a planned design task under tasks-a73dce.
Ideas it wakes: On completion, run `tasks note <id> --no-reviewed "<finding>"` on tasks-3eda29 and tasks-cefa7d before this result's branch merges.
