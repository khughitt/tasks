---
id: tasks-110ba6
title: Establish a consistent plan-step renumbering workflow
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-20T09:10:42Z
updated: 2026-10-20T09:10:42Z
depends: []
parent: tasks-e959ec
tags: []
source: tasks-c9d4ba
agent: codex
---

Question: Can existing tasks operations renumber two linked plan steps and commit the final document and records without intermediate hook failures or lost staging?
Where to start: [ref:brief] gives the report and current contracts; inspect src/commands/edit.rs, src/commands/check.rs, src/record_commit.rs, .githooks/pre-commit and docs/specs/2026-10-10-record-commit-design.md sections 3.1, 3.3 and 3.5.
Bound: One disposable git project with isolated XDG_CONFIG_HOME and XDG_STATE_HOME, a two-step plan, two linked task records and a hook that runs tasks check. Reproduce the reported sequence, then compare the documented project opt-out plus final commit and deliberate staging/deferred commits. Inspect document inclusion, unrelated staged content, warnings and restored configuration; interrupt or fail the second edit and describe recovery. Never change a real project's opt-in, bypass a hook or edit task records by hand. No product-code changes or new CLI surface.
Expected result: Record exact commands, exit codes, warnings and git state on this task and in the brief. Recommend the smallest reliable existing workflow; if none works, state the precise gap and file one planned follow-up with acceptance cases. Keep draft-plan eligibility research tasks-325e79 separate.
Ideas it wakes: On completion, run `tasks note tasks-c9d4ba --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-plan-step-renumbering-brief.md — evidence and alternatives for consistent plan-step renumbering
